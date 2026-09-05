//! The authored path: `assets/world/facts.json`, its validation, and quest packs.
//!
//! Deliberately small. **Most of the base game's facts are minted, not authored**
//! — the city gossiping about its own arrests and notices is the content this
//! feature is tuned on, and it costs no authoring at all. Two rows ship: one to
//! prove the sealed authored path end to end, one to prove `own` and a
//! multi-holder seal.
//!
//! Embedded with `include_str!` exactly as `marks.json` is. A quest pack is *not*
//! embedded: it arrives through [`FactCatalog::extend_from_json`], host-read,
//! because the loaders take `&str` and never a path.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::{FACTS_MAX_LIVE, Fact, GarbleMask, Topic};
use crate::ids::{ActorId, AreaId, FactId, ItemId, is_valid_id};
use crate::knowledge::source::FactSource;
use crate::world::World;

const FACTS_JSON: &str = include_str!("../../../../assets/world/facts.json");

/// The three placeholders a template may name, and nothing else.
const PLACEHOLDERS: [&str; 3] = ["subject", "place", "day"];

/// Fact data that cannot be used without leaving a sentence nobody can render.
#[derive(Debug, Clone, PartialEq)]
pub struct FactCatalogError {
    pub message: String,
}

impl FactCatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FactCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FactCatalogError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactsDoc {
    schema_version: u32,
    #[serde(default, rename = "_doc")]
    _doc: Option<String>,
    #[serde(default, rename = "_topic_doc")]
    _topic_doc: Option<String>,
    facts: Vec<RawFact>,
}

/// Keeping `topic` and `garble` as strings here is what lets the error messages
/// name the consequence instead of serde naming the field.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFact {
    id: String,
    topic: String,
    said: String,
    #[serde(default)]
    own: BTreeMap<String, String>,
    #[serde(default)]
    subject: Vec<String>,
    #[serde(default)]
    seeded: Vec<String>,
    #[serde(default)]
    place: Option<String>,
    #[serde(default)]
    day: Option<i64>,
    #[serde(default = "decays_by_default")]
    decays: bool,
    #[serde(default)]
    garble: Option<String>,
    #[serde(default)]
    source: Option<RawSource>,
}

fn decays_by_default() -> bool {
    true
}

/// Serde's default externally-tagged enum handles the authored spelling with no
/// custom impl: a unit variant matches the bare string `"authored"`, and the
/// newtype and struct variants match `{"custody": "fg2sh"}`, `{"item": "fzbn9"}`
/// and `{"quest_phase": {"quest": "...", "phase": 1}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum RawSource {
    Authored,
    Custody(String),
    Item(String),
    QuestPhase { quest: String, phase: u8 },
}

/// The authored spelling of provenance — the *only* thing outside `source.rs`
/// that names a variant, and it names them by string, not by payload.
///
/// Sealed the way [`FactSource`] is, because it is not a transient: the catalog
/// it sits in lives on `World` for the whole run, and `World` derives `Debug`. A
/// good-faith `Diagnostic(format!("{world:?}"))` would otherwise print a quest
/// pack's `{"quest_phase": …}` — the very payload the resolved field hides — so
/// `Debug` prints a placeholder here too, there is no `Serialize`/`Display`, and
/// the field it sits in is `pub(crate)`: the only reader is [`FactCatalog::seed`].
#[derive(Clone, PartialEq)]
pub enum FactSourceSpec {
    Authored,
    Custody(ActorId),
    Item(ItemId),
    QuestPhase { quest: String, phase: u8 },
}

impl std::fmt::Debug for FactSourceSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FactSourceSpec(<sealed>)")
    }
}

/// One authored row, before a world exists to resolve it against.
#[derive(Debug, Clone, PartialEq)]
pub struct FactSpec {
    pub id: FactId,
    pub topic: Topic,
    pub said: String,
    pub own: BTreeMap<ActorId, String>,
    pub subject: Vec<ActorId>,
    pub seeded: BTreeSet<ActorId>,
    pub place: Option<AreaId>,
    pub day: Option<i64>,
    pub decays: bool,
    pub garble: GarbleMask,
    /// Named here so a quest can bind invalidation without a Rust type; resolved
    /// to a [`FactSource`] by [`FactCatalog::seed`], which is the only reader —
    /// hence `pub(crate)`, so the authored payload is no more reachable from a
    /// host than the resolved one.
    pub(crate) source: FactSourceSpec,
}

impl FactSpec {
    /// Whether the row is bound to nothing — simply so. The one bit of the
    /// authored provenance anything outside the crate may read, the way
    /// [`Fact::is_claimed`] is for the resolved field.
    pub fn is_authored(&self) -> bool {
        matches!(self.source, FactSourceSpec::Authored)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactCatalog {
    specs: Vec<FactSpec>,
}

impl Default for FactCatalog {
    /// The embedded catalog. Panics only if the *compiled-in* asset is malformed,
    /// which is a build-time fact — the same bargain `marks.rs` and `item.rs`
    /// make.
    fn default() -> Self {
        Self::from_embedded().expect("the embedded fact catalog must parse and validate")
    }
}

impl FactCatalog {
    pub fn from_embedded() -> Result<Self, FactCatalogError> {
        Self::from_json(FACTS_JSON)
    }

    pub fn from_json(json: &str) -> Result<Self, FactCatalogError> {
        let mut catalog = Self { specs: Vec::new() };
        catalog.merge(json)?;
        Ok(catalog)
    }

    /// A quest pack, host-supplied. Returns how many rows were added; a duplicate
    /// id is an error, not a silent overwrite — a fact's identity is its id.
    pub fn extend_from_json(&mut self, json: &str) -> Result<usize, FactCatalogError> {
        let before = self.specs.len();
        self.merge(json)?;
        Ok(self.specs.len() - before)
    }

    pub fn specs(&self) -> &[FactSpec] {
        &self.specs
    }

    fn merge(&mut self, json: &str) -> Result<(), FactCatalogError> {
        let doc: FactsDoc = serde_json::from_str(json)
            .map_err(|error| FactCatalogError::new(format!("invalid facts.json: {error}")))?;
        if doc.schema_version != 1 {
            return Err(FactCatalogError::new(format!(
                "unsupported facts schema {}; expected 1",
                doc.schema_version
            )));
        }
        let mut parsed = Vec::with_capacity(doc.facts.len());
        for raw in &doc.facts {
            // Uniqueness is checked against the rows already loaded *and* the ones
            // this document has contributed so far, so a pack cannot shadow a
            // shipped row and a file cannot shadow itself.
            if self.specs.iter().any(|spec| spec.id.as_str() == raw.id)
                || parsed
                    .iter()
                    .any(|spec: &FactSpec| spec.id.as_str() == raw.id)
            {
                return Err(FactCatalogError::new(format!(
                    "duplicate fact id '{}' — a fact's identity is its id, so two rows with \
                     one id would be one fact with two texts",
                    raw.id
                )));
            }
            parsed.push(parse_fact(raw)?);
        }
        self.specs.extend(parsed);
        Ok(())
    }

    /// Install every row into a live world: resolve `place` against the
    /// [`AreaMap`](crate::areas::AreaMap), stamp the handles, and drop rows naming
    /// an actor the world does not have.
    ///
    /// Returns diagnostics — **never a panic**, because a hermetic test world
    /// legitimately lacks the cast and a panic here would make every unit test in
    /// the crate depend on the lore. Dropping the rows nobody in this world holds
    /// is also what keeps the demo seed, every hermetic test and the frozen golden
    /// fixtures byte-identical: none of them contains a lore id.
    pub fn seed(&self, world: &mut World) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for spec in &self.specs {
            let id = spec.id.as_str();

            let place = match &spec.place {
                Some(area) => match world.area_map.key_of_id(area.as_str()) {
                    Some(key) => Some(key),
                    None => {
                        diagnostics.push(format!(
                            "fact {id}: unknown area '{area}'; place left unset"
                        ));
                        None
                    }
                },
                None => None,
            };

            let mut seeded = BTreeSet::new();
            for actor in &spec.seeded {
                if world.characters.contains_key(actor) {
                    seeded.insert(actor.clone());
                } else {
                    diagnostics.push(format!(
                        "fact {id}: seeded actor {actor} is not in this world; dropped from seeded"
                    ));
                }
            }

            // A fact with no resolvable subject cannot render `{subject}`.
            let mut subject_missing = false;
            for actor in &spec.subject {
                if !world.characters.contains_key(actor) {
                    diagnostics.push(format!(
                        "fact {id}: subject {actor} is not in this world; row skipped"
                    ));
                    subject_missing = true;
                }
            }
            if subject_missing {
                continue;
            }

            // An authored fact nobody in this world holds is not a fact of this
            // world. Silent: it is the ordinary case in a hermetic world.
            if seeded.is_empty() {
                continue;
            }

            let source = match &spec.source {
                FactSourceSpec::Authored => FactSource::authored(),
                FactSourceSpec::Custody(who) => FactSource::custody(who.clone()),
                FactSourceSpec::QuestPhase { quest, phase } => {
                    FactSource::quest_phase(quest.clone(), *phase)
                }
                FactSourceSpec::Item(item) => {
                    let holder = world
                        .characters
                        .iter()
                        .find(|(_, character)| character.holds().contains(item))
                        .map(|(actor, _)| actor.clone());
                    match holder {
                        Some(holder) => FactSource::item_with(item.clone(), holder),
                        None => {
                            // The item's id stays out of the line: it is the
                            // binding a source is made of, and this rides
                            // `startup_diagnostics` into `logs.jsonl`.
                            diagnostics.push(format!(
                                "fact {id}: its item is in nobody's hands; source left unbound"
                            ));
                            FactSource::authored()
                        }
                    }
                }
            };

            let own = spec
                .own
                .iter()
                .filter(|(actor, _)| seeded.contains(*actor))
                .map(|(actor, line)| (actor.clone(), line.clone()))
                .collect();

            // Asked before a handle is taken: a second seeding of the same rows
            // (the same catalog twice into one world, or a pack duplicating an
            // installed row) is "already there", not "the store is full", and
            // must not burn a `(FactKey, sequence)` pair on the way to being
            // refused.
            if world.knowledge.key_of(&spec.id).is_some() {
                diagnostics.push(format!("fact {id}: already installed; skipped"));
                continue;
            }

            // The two frozen salience inputs, from the one function that computes
            // them — the authored path and both coded mints must not disagree.
            let (quiet_among, craft_ear) = crate::knowledge::frozen_ears(world, &spec.subject);
            let (key, sequence) = world.knowledge.next_handles();
            let fact = Fact {
                id: spec.id.clone(),
                key,
                sequence,
                subject: spec.subject.clone(),
                place,
                day: spec.day,
                said: spec.said.clone(),
                own,
                seeded,
                garble: spec.garble,
                decays: spec.decays,
                topic: spec.topic,
                // An authored `decays: true` row is news the world starts with, so
                // it starts cooling when the world starts. Left unstamped it would
                // be immortal news — heat 1.0 for the whole run, volunteered
                // forever — which is the bug `HEAT_GONE_BELOW` refuses in the air
                // and the same one in the store. `decays: false` stays unstamped
                // on purpose: standing truth is answerable forever.
                minted_game_days: spec
                    .decays
                    .then(|| world.current_time.map(|time| time.game_days()))
                    .flatten(),
                quiet_among,
                craft_ear,
                source,
            };
            if world.knowledge.install(fact).is_none() {
                diagnostics.push(format!(
                    "fact {id}: the store is full at {FACTS_MAX_LIVE} live facts; not installed"
                ));
            }
        }
        diagnostics
    }
}

/// Every row of `03_assets.md` §1's validation table, in that order.
fn parse_fact(raw: &RawFact) -> Result<FactSpec, FactCatalogError> {
    let id = &raw.id;
    // `id_newtype!`'s `Deserialize` is `transparent` and does not validate, which
    // is why this row exists at all.
    if !is_valid_id(id) {
        // `03_assets.md` writes "1..=64"; the crate's own bound is
        // `MAX_ID_CHARS`, and `InvalidId`'s Display already words it this way, so
        // the number is read off the constant and cannot drift from it.
        return Err(FactCatalogError::new(format!(
            "fact id '{id}' must be 1..={} characters and free of control characters",
            crate::MAX_ID_CHARS
        )));
    }
    let Some(topic) = Topic::parse(&raw.topic) else {
        return Err(FactCatalogError::new(format!(
            "fact {id} has unknown topic '{}'; expected one of {}",
            raw.topic,
            Topic::AUTHORED_LIST
        )));
    };
    if raw.said.trim().is_empty() {
        return Err(FactCatalogError::new(format!(
            "fact {id} has no said text — a fact nobody can say is a fact nobody can hold"
        )));
    }
    let garble = match &raw.garble {
        Some(authored) => GarbleMask::parse(authored)
            .map_err(|error| FactCatalogError::new(format!("fact {id} has {}", error.message)))?,
        None => GarbleMask::NONE,
    };

    if garble.subject && !raw.said.contains("{subject}") {
        return Err(FactCatalogError::new(format!(
            "fact {id} may garble its subject but its said text names no {{subject}} \
             placeholder — the swap would be invisible, so the chain could never be walked back"
        )));
    }
    if garble.place && !raw.said.contains("{place}") {
        return Err(FactCatalogError::new(format!(
            "fact {id} may garble its place but its said text names no {{place}} placeholder"
        )));
    }
    if garble.day && !raw.said.contains("{day}") {
        return Err(FactCatalogError::new(format!(
            "fact {id} may garble its day but its said text names no {{day}} placeholder"
        )));
    }

    let seeded: BTreeSet<ActorId> = raw
        .seeded
        .iter()
        .map(|actor| ActorId::from_raw(actor.clone()))
        .collect();
    for actor in raw.own.keys() {
        if !raw.seeded.iter().any(|seeded| seeded == actor) {
            return Err(FactCatalogError::new(format!(
                "fact {id} gives {actor} an own line but does not seed them — a first-person \
                 telling belongs to somebody who was there"
            )));
        }
    }

    // Every template, not only `said`: an `own` line naming `{subject}` with
    // nothing to put in it would leave the literal braces on a sheet.
    if raw.subject.is_empty()
        && std::iter::once(&raw.said)
            .chain(raw.own.values())
            .any(|template| template.contains("{subject}"))
    {
        return Err(FactCatalogError::new(format!(
            "fact {id} names a {{subject}} placeholder but has no subject to put in it"
        )));
    }

    for template in std::iter::once(&raw.said).chain(raw.own.values()) {
        match brace_fault(template) {
            Some(BraceFault::Unknown(unknown)) => {
                return Err(FactCatalogError::new(format!(
                    "fact {id} uses unknown placeholder '{unknown}'; only {{subject}}, \
                     {{place}} and {{day}} are substituted"
                )));
            }
            Some(BraceFault::Unclosed) => {
                return Err(FactCatalogError::new(format!(
                    "fact {id} has an unclosed '{{' — a placeholder that never closes would \
                     reach a sheet as a literal brace"
                )));
            }
            None => {}
        }
        // `render_line` fills `{subject}` with a role phrase, so a possessive
        // after it is unreadable.
        if template.contains("{subject}'s") || template.contains("{subject}\u{2019}s") {
            return Err(FactCatalogError::new(format!(
                "fact {id} puts {{subject}} in the possessive — an unknown subject renders as \
                 \"a chandler of the Wick Ward (you don't know their name)\", and \"…'s\" after \
                 that is unreadable"
            )));
        }
        if template.contains("%s") {
            return Err(FactCatalogError::new(format!(
                "fact {id} contains a bare %s, which is the hedge's own placeholder — \
                 substitution would run twice"
            )));
        }
    }

    let source = match &raw.source {
        None | Some(RawSource::Authored) => FactSourceSpec::Authored,
        Some(RawSource::Custody(who)) => FactSourceSpec::Custody(ActorId::from_raw(who.clone())),
        Some(RawSource::Item(item)) => FactSourceSpec::Item(ItemId::from_raw(item.clone())),
        Some(RawSource::QuestPhase { quest, phase }) => FactSourceSpec::QuestPhase {
            quest: quest.clone(),
            phase: *phase,
        },
    };

    Ok(FactSpec {
        id: FactId::from_raw(id.clone()),
        topic,
        said: raw.said.clone(),
        own: raw
            .own
            .iter()
            .map(|(actor, line)| (ActorId::from_raw(actor.clone()), line.clone()))
            .collect(),
        subject: raw
            .subject
            .iter()
            .map(|actor| ActorId::from_raw(actor.clone()))
            .collect(),
        seeded,
        place: raw
            .place
            .as_ref()
            .map(|area| AreaId::from_raw(area.clone())),
        day: raw.day,
        decays: raw.decays,
        garble,
        source,
    })
}

/// What is wrong with a template's braces, when something is.
enum BraceFault {
    /// A `{…}` group naming something the renderer does not substitute.
    Unknown(String),
    /// A `{` with no `}` after it — which would reach a sheet as a literal.
    Unclosed,
}

/// The first brace fault in `template`: an unknown `{…}` group, or an unclosed
/// `{`. An unclosed brace is a fault of its own and never "no placeholder": the
/// text after it is exactly what a sheet would print.
fn brace_fault(template: &str) -> Option<BraceFault> {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return Some(BraceFault::Unclosed);
        };
        let name = &after[..close];
        if !PLACEHOLDERS.contains(&name) {
            return Some(BraceFault::Unknown(name.to_string()));
        }
        rest = &after[close + 1..];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T13. The shipped catalog is the two authored rows, and it validates.
    #[test]
    fn the_embedded_catalog_parses_and_validates() {
        let catalog = FactCatalog::from_embedded().expect("the shipped catalog loads");
        assert_eq!(catalog.specs().len(), 2);
        let ids: Vec<&str> = catalog
            .specs()
            .iter()
            .map(|spec| spec.id.as_str())
            .collect();
        assert_eq!(ids, ["ashe.salt.short", "vell.stall.pitch"]);
        assert_eq!(catalog.specs()[0].topic, Topic::Coin);
        assert_eq!(catalog.specs()[1].topic, Topic::Craft);
        // The seal the shipped rows prove: `own` lines only for people who were
        // there, and one seeded holder with no `own` line at all — she is the one
        // who renders the unknown-people rule on the real city.
        assert_eq!(catalog.specs()[1].seeded.len(), 3);
        assert_eq!(catalog.specs()[1].own.len(), 2);
    }

    fn refuse(json: &str) -> String {
        FactCatalog::from_json(json)
            .expect_err("the row must be refused")
            .message
    }

    fn one(row: &str) -> String {
        format!("{{\"schema_version\": 1, \"facts\": [{row}]}}")
    }

    /// T14. One case per row of `03_assets.md` §1's validation table, plus the
    /// two the possessive and the bare `%s` add, each asserting a substring of
    /// the exact message — every message names the consequence, not the field.
    #[test]
    fn every_loader_rejection_names_its_consequence() {
        let error = refuse("{");
        assert!(error.contains("invalid facts.json:"), "{error}");

        let error = refuse("{\"schema_version\": 2, \"facts\": []}");
        assert!(error.contains("unsupported facts schema 2"), "{error}");

        let error = refuse(&format!(
            "{{\"schema_version\": 1, \"facts\": [{}, {}]}}",
            r#"{"id": "a.b.c", "topic": "law", "said": "x"}"#,
            r#"{"id": "a.b.c", "topic": "law", "said": "y"}"#
        ));
        assert!(error.contains("duplicate fact id 'a.b.c'"), "{error}");
        assert!(error.contains("one fact with two texts"), "{error}");

        // Serde refuses a raw control character in a string before the loader ever
        // sees it, so the reachable half of this row is the length: an id past
        // `MAX_ID_CHARS` is valid JSON and an invalid id.
        let long = "x".repeat(crate::MAX_ID_CHARS + 1);
        let error = refuse(&one(&format!(
            r#"{{"id": "{long}", "topic": "law", "said": "x"}}"#
        )));
        assert!(
            error.contains(&format!(
                "must be 1..={} characters and free of control characters",
                crate::MAX_ID_CHARS
            )),
            "{error}"
        );

        let error = refuse(&one(r#"{"id": "a.b", "topic": "word", "said": "x"}"#));
        assert!(error.contains("has unknown topic 'word'"), "{error}");
        assert!(
            error.contains("bed, blood, law, omen, stranger, coin, bread, craft, talk"),
            "{error}"
        );

        let error = refuse(&one(r#"{"id": "a.b", "topic": "law", "said": "  "}"#));
        assert!(error.contains("has no said text"), "{error}");
        assert!(error.contains("nobody can hold"), "{error}");

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "garble": "colour"}"#,
        ));
        assert!(
            error.contains(
                "has unknown garble field 'colour'; expected none, subject, place or day"
            ),
            "{error}"
        );
        // An empty mask and `none` beside a field both read as one thing and
        // mean another, so neither loads.
        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "garble": ""}"#,
        ));
        assert!(error.contains("has unknown garble field ''"), "{error}");
        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "{day}", "garble": "none,day"}"#,
        ));
        assert!(
            error.contains("has garble 'none,day' that mixes none with a field"),
            "{error}"
        );

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "garble": "subject", "subject": ["q"]}"#,
        ));
        assert!(error.contains("may garble its subject"), "{error}");
        assert!(error.contains("could never be walked back"), "{error}");

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "garble": "place"}"#,
        ));
        assert!(
            error.contains("may garble its place but its said text names no {place} placeholder"),
            "{error}"
        );

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "garble": "day"}"#,
        ));
        assert!(
            error.contains("may garble its day but its said text names no {day} placeholder"),
            "{error}"
        );

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "own": {"q": "mine"}}"#,
        ));
        assert!(
            error.contains("gives q an own line but does not seed them"),
            "{error}"
        );
        assert!(error.contains("somebody who was there"), "{error}");

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "{subject} did it"}"#,
        ));
        assert!(
            error.contains("names a {subject} placeholder but has no subject to put in it"),
            "{error}"
        );

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "at {hour} it happened"}"#,
        ));
        assert!(error.contains("uses unknown placeholder 'hour'"), "{error}");
        assert!(
            error.contains("only {subject}, {place} and {day} are substituted"),
            "{error}"
        );

        // An unclosed brace is not "no placeholder": it is a literal brace on a
        // sheet. In `said` and in an `own` line alike.
        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "{subject was seen at the ford", "subject": ["q"]}"#,
        ));
        assert!(error.contains("has an unclosed '{'"), "{error}");
        assert!(error.contains("literal brace"), "{error}");
        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "x", "own": {"q": "I saw {place"}, "seeded": ["q"]}"#,
        ));
        assert!(error.contains("has an unclosed '{'"), "{error}");

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "{subject}'s beam", "subject": ["q"]}"#,
        ));
        assert!(
            error.contains("puts {subject} in the possessive"),
            "{error}"
        );

        let error = refuse(&one(
            r#"{"id": "a.b", "topic": "law", "said": "a %s beam"}"#,
        ));
        assert!(error.contains("contains a bare %s"), "{error}");
        assert!(error.contains("substitution would run twice"), "{error}");
    }

    /// T15. A quest pack merges, and a duplicate id is an error naming the id
    /// rather than a silent overwrite.
    #[test]
    fn a_quest_pack_merges_and_refuses_a_duplicate_id() {
        let mut catalog = FactCatalog::default();
        let added = catalog
            .extend_from_json(&one(
                r#"{"id": "quest.bale.weight", "topic": "coin", "said": "the bale was light"}"#,
            ))
            .expect("a new id merges");
        assert_eq!(added, 1);
        assert_eq!(catalog.specs().len(), 3);

        let error = catalog
            .extend_from_json(&one(
                r#"{"id": "ashe.salt.short", "topic": "coin", "said": "different words"}"#,
            ))
            .expect_err("a shipped id is refused")
            .message;
        assert!(
            error.contains("duplicate fact id 'ashe.salt.short'"),
            "{error}"
        );
        // The refused pack changed nothing.
        assert_eq!(catalog.specs().len(), 3);
    }

    /// T16. With no cast, nothing installs — which is the mechanism that keeps
    /// every hermetic world and every frozen golden fixture byte-identical.
    #[test]
    fn seed_skips_a_row_whose_cast_this_world_lacks() {
        let mut world = World::new();
        let diagnostics = FactCatalog::default().seed(&mut world);
        assert!(world.knowledge.is_empty(), "{diagnostics:?}");
        // Two rows, each reporting its unknown area and its absent subject.
        assert!(
            diagnostics
                .iter()
                .any(|line| line
                    .contains("fact ashe.salt.short: subject fg2sh is not in this world")),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|line| line
                    .contains("fact vell.stall.pitch: subject dv8ll is not in this world")),
            "{diagnostics:?}"
        );
    }

    /// Seeding the same rows twice — the same catalog again, or a pack
    /// duplicating an installed row — is "already installed", not "the store is
    /// full", and it burns no handle: the next new row still takes the next key.
    #[test]
    fn seeding_an_installed_row_again_is_skipped_without_a_handle() {
        let mut world = World::new();
        world.add_character(crate::character::Character::from_sheet(
            crate::character::CharacterSheet {
                pockets: Vec::new(),
                frontbutt: None,
                id: ActorId::from_raw("wit"),
                name: "The Witness".into(),
                control: crate::character::Control::Llm,
                back_story: String::new(),
                location_description: String::new(),
                appearance: Default::default(),
                voice_key: None,
                position_m: crate::math::Vec3::new(0.0, 0.91, 0.0),
                facing_yaw: 0.0,
                holds: Vec::new(),
                goal: crate::GOAL_NONE.into(),
                memories: Vec::new(),
                knows: BTreeSet::new(),
                lore: None,
                presence: crate::character::Presence::InCity,
                presence_epoch: 0,
                economic_class: crate::character::EconomicClass::Resident,
            },
        ));
        let first = FactCatalog::from_json(&one(
            r#"{"id": "test.twice", "topic": "law", "said": "x", "seeded": ["wit"]}"#,
        ))
        .expect("the row parses");
        assert!(first.seed(&mut world).is_empty());
        let diagnostics = first.seed(&mut world);
        assert_eq!(
            diagnostics,
            vec!["fact test.twice: already installed; skipped".to_string()]
        );
        assert_eq!(world.knowledge.len(), 1);

        let next = FactCatalog::from_json(&one(
            r#"{"id": "test.next", "topic": "law", "said": "y", "seeded": ["wit"]}"#,
        ))
        .expect("the row parses");
        assert!(next.seed(&mut world).is_empty());
        assert_eq!(
            world.knowledge.key_of(&FactId::from_raw("test.next")),
            Some(crate::ids::FactKey(1)),
            "the refused second seeding took no handle"
        );
    }
}
