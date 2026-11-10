//! Inquiries: a matter somebody can look into, the accounts lodged with its
//! reviewer, and the finding a dated review gives
//! (`M4_accounts_inquiries_findings.md`).
//!
//! An inquiry reaches a world in a **case pack**: fact rows in the `facts.json`
//! shape, the retellings already abroad when the world begins, and the
//! inquiries about them. [`install`] is the only way in, and it is the strict
//! counterpart of [`FactCatalog::seed`]: a row naming somebody, somewhere or
//! something this world lacks is refused with the field and the id, where
//! `seed` would skip it with a diagnostic.
//!
//! A pack is fresh-world content (D02). Its rules are copied into the saved
//! [`Inquiry`], so a restored world carries its inquiries and reads no pack.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::clock::{Office, WorldTime};
use crate::ids::{ActorId, FactId};
use crate::knowledge::catalog::{RawFact, parse_fact};
use crate::knowledge::{self, DAY_OFFSET_MAX, FactCatalog, FactSpec, FactView, Telling};
use crate::world::World;

/// How many different facts the world's inquiries may list between them. An
/// account of one is kept on the player's receipt for good, and the receipts
/// are capped at [`knowledge::PLAYER_RECEIPTS_MAX`], so the kept ones must
/// leave most of that room to ordinary hearsay.
pub const INQUIRY_FACTS_MAX: usize = 24;

/// One matter under inquiry. Everything down to `full_finding` is the pack's
/// rule, copied into the save; the rest is what play has done with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inquiry {
    pub id: String,
    pub title: String,
    /// Who takes the accounts and holds the review.
    pub reviewer: ActorId,
    /// The office at whose ringing a lodged bundle is reviewed.
    pub review_office: Office,
    /// The facts an account must be about to bear on this matter.
    pub facts: Vec<FactId>,
    /// How many people who were there a full finding must trace back to.
    pub sources_needed: u8,
    /// What the reviewer finds when the bundle lacks nothing.
    pub full_finding: String,
    pub lodged: Vec<Lodged>,
    /// Game days at which the next review falls, while a bundle awaits one.
    pub review_due: Option<f64>,
    pub finding: Option<Finding>,
}

/// One account as it was lodged: a copy, so a review reads the bundle and
/// nothing its speaker has heard since.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lodged {
    pub fact: FactId,
    pub speaker: ActorId,
    /// Whose eyes the account rests on, when the speaker could say.
    pub origin: Option<ActorId>,
    pub view: FactView,
}

/// What a review made of the bundle. Full when nothing is `missing`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub held: WorldTime,
    pub accounts: u8,
    pub sources: u8,
    pub missing: Vec<Support>,
}

/// A kind of support a bundle can lack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Support {
    IndependentSource,
    SettlingWord,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPack {
    schema_version: u32,
    #[serde(default, rename = "_doc")]
    _doc: Option<String>,
    facts: Vec<RawFact>,
    #[serde(default)]
    retold: Vec<RawRetold>,
    #[serde(default)]
    inquiries: Vec<RawInquiry>,
}

/// Somebody who was not there and holds the fact because `from` told them,
/// `day_offset` days out if the telling moved the day.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRetold {
    fact: String,
    by: String,
    from: String,
    day_offset: i8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInquiry {
    id: String,
    title: String,
    reviewer: String,
    review_office: String,
    facts: Vec<String>,
    sources_needed: u8,
    finding: String,
}

/// Install one case pack into a fresh world: its facts, then its retellings,
/// then its inquiries.
///
/// Every check runs before the world is touched, and a refusal says which
/// field of the pack and which id. The fact, actor and place checks apply to
/// every row. Only a fact an inquiry lists must also be standing and authored,
/// so a pack with no inquiries is a plain fact pack and may carry news.
///
/// The fact store can still turn a checked row away, when it is full of
/// standing facts. That is an error too, from a world part-installed and not
/// to be run.
pub fn install(world: &mut World, json: &str) -> Result<(), String> {
    let pack: RawPack =
        serde_json::from_str(json).map_err(|error| format!("does not parse: {error}"))?;
    if pack.schema_version != 1 {
        return Err(format!(
            "schema_version {} is not supported; expected 1",
            pack.schema_version
        ));
    }

    let mut specs: Vec<FactSpec> = Vec::with_capacity(pack.facts.len());
    for (index, raw) in pack.facts.iter().enumerate() {
        let spec = parse_fact(raw).map_err(|error| format!("facts[{index}]: {error}"))?;
        let id = &spec.id;
        if world.knowledge.key_of(id).is_some() || specs.iter().any(|earlier| earlier.id == *id) {
            return Err(format!(
                "facts[{index}].id '{id}' is already taken by another fact"
            ));
        }
        if spec.seeded.is_empty() {
            return Err(format!(
                "facts[{index}].seeded of '{id}' is empty: a fact nobody was there for cannot be \
                 told"
            ));
        }
        // `own` needs no pass of its own: `parse_fact` keeps it inside `seeded`.
        for actor in &spec.seeded {
            person(world, format_args!("facts[{index}].seeded"), actor.as_str())?;
        }
        for actor in &spec.subject {
            person(
                world,
                format_args!("facts[{index}].subject"),
                actor.as_str(),
            )?;
        }
        if let Some(place) = &spec.place
            && world.area_map.key_of_id(place.as_str()).is_none()
        {
            return Err(format!(
                "facts[{index}].place '{place}' is not an area of this world"
            ));
        }
        specs.push(spec);
    }

    // Who holds each fact once the pack is in, and at how many removes.
    let mut holders: BTreeMap<(usize, ActorId), u8> = specs
        .iter()
        .enumerate()
        .flat_map(|(fact, spec)| {
            spec.seeded
                .iter()
                .map(move |actor| ((fact, actor.clone()), 0))
        })
        .collect();
    let mut retellings: Vec<(usize, ActorId, Telling)> = Vec::with_capacity(pack.retold.len());
    for (index, row) in pack.retold.iter().enumerate() {
        let Some(fact) = specs
            .iter()
            .position(|spec| spec.id.as_str() == row.fact.as_str())
        else {
            return Err(format!(
                "retold[{index}].fact '{}' is not a fact of this pack",
                row.fact
            ));
        };
        let spec = &specs[fact];
        let by = person(world, format_args!("retold[{index}].by"), &row.by)?;
        let from = person(world, format_args!("retold[{index}].from"), &row.from)?;
        let Some(&from_hops) = holders.get(&(fact, from.clone())) else {
            return Err(format!(
                "retold[{index}].from '{from}' does not hold '{}': seed them, or put the row that \
                 tells them first",
                spec.id
            ));
        };
        if holders.contains_key(&(fact, by.clone())) {
            return Err(format!(
                "retold[{index}].by '{by}' already holds '{}'",
                spec.id
            ));
        }
        if row.day_offset != 0 {
            if !(-DAY_OFFSET_MAX..=DAY_OFFSET_MAX).contains(&row.day_offset) {
                return Err(format!(
                    "retold[{index}].day_offset {} is outside -{DAY_OFFSET_MAX}..={DAY_OFFSET_MAX}",
                    row.day_offset
                ));
            }
            if spec.day.is_none() || !spec.said.contains("{day}") {
                return Err(format!(
                    "retold[{index}].day_offset {} needs '{}' to carry a day and name {{day}} in \
                     its said text",
                    row.day_offset, spec.id
                ));
            }
        }
        let hops = from_hops.saturating_add(1);
        holders.insert((fact, by.clone()), hops);
        retellings.push((
            fact,
            by,
            Telling {
                hops,
                from: Some(from),
                heat: 1.0,
                view: FactView {
                    day_offset: row.day_offset,
                    ..FactView::default()
                },
            },
        ));
    }

    let mut inquiries: Vec<Inquiry> = Vec::with_capacity(pack.inquiries.len());
    let mut listed: BTreeSet<FactId> = world
        .inquiries
        .iter()
        .flat_map(|inquiry| inquiry.facts.iter().cloned())
        .collect();
    for (index, raw) in pack.inquiries.iter().enumerate() {
        let id = &raw.id;
        if world
            .inquiries
            .iter()
            .chain(&inquiries)
            .any(|inquiry| inquiry.id == *id)
        {
            return Err(format!(
                "inquiries[{index}].id '{id}' is already an inquiry"
            ));
        }
        let reviewer = person(
            world,
            format_args!("inquiries[{index}].reviewer"),
            &raw.reviewer,
        )?;
        let Some(review_office) = Office::from_config_name(&raw.review_office) else {
            return Err(format!(
                "inquiries[{index}].review_office '{}' is not an office: try watch, kindling, \
                 dayspring, high_wick, waning, lamplight or snuffing",
                raw.review_office
            ));
        };
        if raw.facts.is_empty() {
            return Err(format!(
                "inquiries[{index}].facts of '{id}' is empty: an inquiry is about something"
            ));
        }
        let mut facts = Vec::with_capacity(raw.facts.len());
        let mut witnesses: BTreeSet<&ActorId> = BTreeSet::new();
        for fact in &raw.facts {
            let Some(spec) = specs.iter().find(|spec| spec.id.as_str() == fact.as_str()) else {
                return Err(format!(
                    "inquiries[{index}].facts '{fact}' is not a fact of this pack"
                ));
            };
            // An account of it is kept for good, so the fact must be one the
            // store never evicts and the world never stops bearing out.
            if spec.decays || !spec.is_authored() {
                return Err(format!(
                    "inquiries[{index}].facts '{fact}' must be a standing authored fact: \
                     \"decays\": false and no source binding"
                ));
            }
            witnesses.extend(&spec.seeded);
            facts.push(spec.id.clone());
        }
        if raw.sources_needed == 0 || usize::from(raw.sources_needed) > witnesses.len() {
            return Err(format!(
                "inquiries[{index}].sources_needed {} must be between 1 and {}, the number of \
                 people seeded with the facts of '{id}'",
                raw.sources_needed,
                witnesses.len()
            ));
        }
        if raw.finding.trim().is_empty() {
            return Err(format!("inquiries[{index}].finding of '{id}' is empty"));
        }
        listed.extend(facts.iter().cloned());
        if listed.len() > INQUIRY_FACTS_MAX {
            return Err(format!(
                "inquiries[{index}].facts of '{id}' brings the world to {} facts under inquiry; \
                 the limit is {INQUIRY_FACTS_MAX}",
                listed.len()
            ));
        }
        inquiries.push(Inquiry {
            id: id.clone(),
            title: raw.title.clone(),
            reviewer,
            review_office,
            facts,
            sources_needed: raw.sources_needed,
            full_finding: raw.finding.clone(),
            lodged: Vec::new(),
            review_due: None,
            finding: None,
        });
    }

    let catalog = FactCatalog::from_specs(specs);
    // Everything `seed` would skip was refused above, so a diagnostic here is
    // the store being full or a source bound to nothing, and both are refusals.
    if let Some(line) = catalog.seed(world).into_iter().next() {
        return Err(line);
    }
    for (fact, by, telling) in retellings {
        let id = &catalog.specs()[fact].id;
        let key = world
            .knowledge
            .key_of(id)
            .ok_or_else(|| format!("fact '{id}' was not installed"))?;
        // No clock on the telling: a retelling the world begins with never
        // cools, so it is the last thing ordinary gossip crowds out of its
        // holder.
        knowledge::learn(world, &by, key, telling, None);
    }
    world.inquiries.extend(inquiries);
    Ok(())
}

/// `id` as somebody of this world, or the refusal naming the pack's `field`.
fn person(world: &World, field: std::fmt::Arguments<'_>, id: &str) -> Result<ActorId, String> {
    let actor = ActorId::from_raw(id);
    if world.characters.contains_key(&actor) {
        Ok(actor)
    } else {
        Err(format!("{field} '{id}' is not a person in this world"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::character::{Character, CharacterSheet, Control, EconomicClass, Presence};
    use crate::math::Vec3;

    fn actor(id: &str) -> ActorId {
        ActorId::from_raw(id)
    }

    /// A hermetic world of four: two witnesses, a listener and a reviewer.
    fn world() -> World {
        let mut world = World::new();
        for id in ["ada", "bem", "cob", "rev"] {
            world.add_character(Character::from_sheet(CharacterSheet {
                pockets: Vec::new(),
                frontbutt: None,
                id: actor(id),
                name: id.to_string(),
                control: Control::Llm,
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
                lore: None,
                presence: Presence::InCity,
                presence_epoch: 0,
                economic_class: EconomicClass::Resident,
            }));
        }
        world
    }

    fn pack() -> Value {
        json!({
            "schema_version": 1,
            "facts": [{
                "id": "test.cart",
                "topic": "talk",
                "said": "the cart went over at the ford {day} with nobody on the box",
                "seeded": ["ada", "bem"],
                "day": 0,
                "decays": false
            }],
            "retold": [{"fact": "test.cart", "by": "cob", "from": "ada", "day_offset": -1}],
            "inquiries": [{
                "id": "test_cart",
                "title": "The cart at the ford",
                "reviewer": "rev",
                "review_office": "high_wick",
                "facts": ["test.cart"],
                "sources_needed": 2,
                "finding": "The cart went over by itself."
            }]
        })
    }

    /// The refusal of [`pack`] after `edit`, having checked that the world,
    /// its fact store and inquiries included, is exactly as it was.
    fn refused(edit: impl FnOnce(&mut Value)) -> String {
        let mut world = world();
        let before = world.clone();
        let mut pack = pack();
        edit(&mut pack);
        let error = install(&mut world, &pack.to_string()).expect_err("the pack is refused");
        assert_eq!(world, before, "a refused pack leaves the world alone");
        error
    }

    #[test]
    fn a_good_pack_installs_its_facts_retellings_and_inquiries() {
        let mut world = world();
        install(&mut world, &pack().to_string()).expect("the pack installs");

        let fact = FactId::from_raw("test.cart");
        let witness = knowledge::holds(&world, &actor("ada"), &fact).expect("seeded");
        assert!(witness.is_first_hand());
        let told = knowledge::holds(&world, &actor("cob"), &fact).expect("retold");
        assert_eq!(
            (told.hops, told.from, told.view.day_offset),
            (1, Some(actor("ada")), -1)
        );
        assert!(knowledge::holds(&world, &actor("rev"), &fact).is_none());
        assert_eq!(
            world.inquiries,
            vec![Inquiry {
                id: "test_cart".to_string(),
                title: "The cart at the ford".to_string(),
                reviewer: actor("rev"),
                review_office: Office::HighWick,
                facts: vec![fact],
                sources_needed: 2,
                full_finding: "The cart went over by itself.".to_string(),
                lodged: Vec::new(),
                review_due: None,
                finding: None,
            }]
        );
    }

    #[test]
    fn a_pack_with_no_inquiries_may_carry_a_decaying_fact() {
        let mut world = world();
        let pack = json!({
            "schema_version": 1,
            "facts": [{
                "id": "test.ford",
                "topic": "talk",
                "said": "the ford is running high",
                "seeded": ["ada"]
            }]
        });
        install(&mut world, &pack.to_string()).expect("a plain fact pack installs");
        let fact = world
            .knowledge
            .fact_by_id(&FactId::from_raw("test.ford"))
            .expect("installed");
        assert!(fact.decays);
        assert!(world.inquiries.is_empty());
    }

    #[test]
    fn a_pack_already_in_the_world_is_refused() {
        let mut world = world();
        install(&mut world, &pack().to_string()).expect("the first time");
        let before = world.clone();
        let error = install(&mut world, &pack().to_string()).expect_err("not twice");
        assert_eq!(
            error,
            "facts[0].id 'test.cart' is already taken by another fact"
        );
        assert_eq!(world, before);
    }

    #[test]
    fn an_unknown_reviewer_is_refused() {
        let error = refused(|pack| pack["inquiries"][0]["reviewer"] = "zz999".into());
        assert_eq!(
            error,
            "inquiries[0].reviewer 'zz999' is not a person in this world"
        );
    }

    #[test]
    fn an_unknown_seeded_actor_is_refused() {
        let error = refused(|pack| pack["facts"][0]["seeded"][1] = "zz999".into());
        assert_eq!(
            error,
            "facts[0].seeded 'zz999' is not a person in this world"
        );
    }

    #[test]
    fn an_unknown_place_is_refused() {
        let error = refused(|pack| pack["facts"][0]["place"] = "nowhere".into());
        assert_eq!(
            error,
            "facts[0].place 'nowhere' is not an area of this world"
        );
    }

    #[test]
    fn a_retelling_from_somebody_who_does_not_hold_the_fact_is_refused() {
        let error = refused(|pack| pack["retold"][0]["from"] = "rev".into());
        assert_eq!(
            error,
            "retold[0].from 'rev' does not hold 'test.cart': seed them, or put the row that tells \
             them first"
        );
        let error = refused(|pack| pack["retold"][0]["by"] = "bem".into());
        assert_eq!(error, "retold[0].by 'bem' already holds 'test.cart'");
    }

    #[test]
    fn a_day_offset_needs_a_dated_fact_that_says_its_day() {
        let error = refused(|pack| {
            pack["facts"][0]["said"] = "the cart went over at the ford".into();
        });
        assert_eq!(
            error,
            "retold[0].day_offset -1 needs 'test.cart' to carry a day and name {day} in its said \
             text"
        );
        let error = refused(|pack| pack["retold"][0]["day_offset"] = 4.into());
        assert_eq!(error, "retold[0].day_offset 4 is outside -3..=3");
    }

    #[test]
    fn an_inquiry_about_a_fact_outside_the_pack_is_refused() {
        let error = refused(|pack| pack["inquiries"][0]["facts"] = json!(["test.other"]));
        assert_eq!(
            error,
            "inquiries[0].facts 'test.other' is not a fact of this pack"
        );
    }

    #[test]
    fn an_inquiry_about_a_decaying_fact_is_refused() {
        let error = refused(|pack| pack["facts"][0]["decays"] = true.into());
        assert_eq!(
            error,
            "inquiries[0].facts 'test.cart' must be a standing authored fact: \"decays\": false \
             and no source binding"
        );
    }

    #[test]
    fn a_duplicate_id_is_refused() {
        let error = refused(|pack| {
            let again = pack["facts"][0].clone();
            pack["facts"].as_array_mut().expect("rows").push(again);
        });
        assert_eq!(
            error,
            "facts[1].id 'test.cart' is already taken by another fact"
        );
        let error = refused(|pack| {
            let again = pack["inquiries"][0].clone();
            pack["inquiries"].as_array_mut().expect("rows").push(again);
        });
        assert_eq!(error, "inquiries[1].id 'test_cart' is already an inquiry");
    }

    #[test]
    fn a_review_office_that_is_no_office_is_refused() {
        let error = refused(|pack| pack["inquiries"][0]["review_office"] = "noon".into());
        assert_eq!(
            error,
            "inquiries[0].review_office 'noon' is not an office: try watch, kindling, dayspring, \
             high_wick, waning, lamplight or snuffing"
        );
    }

    #[test]
    fn sources_needed_is_bounded_by_the_seeded_witnesses() {
        for asked in [0, 3] {
            let error = refused(|pack| pack["inquiries"][0]["sources_needed"] = asked.into());
            assert_eq!(
                error,
                format!(
                    "inquiries[0].sources_needed {asked} must be between 1 and 2, the number of \
                     people seeded with the facts of 'test_cart'"
                )
            );
        }
    }

    #[test]
    fn the_facts_under_inquiry_are_capped() {
        let ids: Vec<String> = (0..=INQUIRY_FACTS_MAX)
            .map(|n| format!("test.fact{n}"))
            .collect();
        let error = refused(|pack| {
            pack["facts"] = ids
                .iter()
                .map(|id| {
                    json!({
                        "id": id,
                        "topic": "talk",
                        "said": "x",
                        "seeded": ["ada"],
                        "decays": false
                    })
                })
                .collect();
            pack["retold"] = json!([]);
            pack["inquiries"][0]["facts"] = json!(ids);
            pack["inquiries"][0]["sources_needed"] = 1.into();
        });
        assert_eq!(
            error,
            "inquiries[0].facts of 'test_cart' brings the world to 25 facts under inquiry; the \
             limit is 24"
        );
    }

    #[test]
    fn an_unknown_field_or_schema_is_refused() {
        let error = refused(|pack| pack["verdict"] = "guilty".into());
        assert!(
            error.starts_with("does not parse: unknown field `verdict`"),
            "{error}"
        );
        let error = refused(|pack| pack["schema_version"] = 2.into());
        assert_eq!(error, "schema_version 2 is not supported; expected 1");
    }
}
