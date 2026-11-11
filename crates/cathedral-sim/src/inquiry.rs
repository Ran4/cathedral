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
use crate::error::{ActionError, ActionErrorCode};
use crate::ids::{ActorId, FactId, FactKey};
use crate::knowledge::catalog::{RawFact, parse_fact};
use crate::knowledge::{
    self, DAY_OFFSET_MAX, FactCatalog, FactSpec, FactView, KNOWN_SHEET_MAX, Telling,
};
use crate::perception::{cap_first, identify};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Support {
    /// Fewer people who were there stand behind the bundle than the matter
    /// needs.
    IndependentSource,
    /// Accounts of one fact disagree, and none is from somebody who was there.
    SettlingWord,
}

impl Support {
    /// What a bundle lacking this is still wanting. The words come from the
    /// kind alone, so they cannot name a person or anything the reviewer was
    /// not given (D04).
    pub fn wanting(self) -> &'static str {
        match self {
            Support::IndependentSource => {
                "a further account that does not trace back to the same person"
            }
            Support::SettlingWord => {
                "the word of someone who was there, to settle accounts that disagree"
            }
        }
    }
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

/// Which of the facts seated on `speaker`'s sheet they could give the player
/// an account of: those an inquiry is about, while the player is in earshot.
///
/// `seated` is cut to [`KNOWN_SHEET_MAX`] before the filter, as
/// [`knowledge::Knowledge::note_seated`] cuts what it keeps. The prompt asks
/// with the whole relevance list and the `recount` verb with the kept one, so
/// the sheet offers the verb for exactly the facts the verb will find.
pub fn recountable(world: &World, speaker: &ActorId, seated: &[FactKey]) -> Vec<FactKey> {
    if world.inquiries.is_empty() || knowledge::player_in_earshot(world, speaker).is_none() {
        return Vec::new();
    }
    let under_inquiry = |fact: &FactId| world.inquiries.iter().any(|i| i.facts.contains(fact));
    seated
        .iter()
        .take(KNOWN_SHEET_MAX)
        .copied()
        .filter(|key| {
            world
                .knowledge
                .fact(*key)
                .is_some_and(|fact| under_inquiry(&fact.id))
        })
        .collect()
}

/// [`knowledge::recount`] for each of `keys`: the words of every account
/// recorded, or the first refusal when none was. A refusal has therefore
/// recorded nothing, whichever key it came from.
pub(crate) fn recount_all(
    world: &mut World,
    speaker: &ActorId,
    keys: &[FactKey],
) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut refusal = None;
    for key in keys {
        match knowledge::recount(world, speaker, *key) {
            Ok(word) => words.push(word),
            Err(reason) => refusal = refusal.or(Some(reason)),
        }
    }
    match refusal {
        Some(reason) if words.is_empty() => Err(reason),
        _ => Ok(words),
    }
}

/// The player's ask control: `target` gives their account of every fact of one
/// inquiry that they hold, and nothing is said aloud.
///
/// It records through [`knowledge::recount`], exactly as the `recount` verb
/// does after a spoken answer, so asking by the control and asking in words
/// leave the same account. The target is told they were asked. A refusal names
/// its own reason and leaves the world as it was.
pub fn ask(world: &mut World, target: &ActorId, inquiry_id: &str) -> Result<String, ActionError> {
    let Some(inquiry) = world.inquiries.iter().find(|i| i.id == inquiry_id) else {
        return Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            "nobody is looking into any such matter",
        ));
    };
    let (Some(player), Some(them)) = (world.player_id(), world.characters.get(target)) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownTarget,
            "there is nobody by that id to ask",
        ));
    };
    let name = cap_first(&identify(&world.characters[player], them));
    if knowledge::player_in_earshot(world, target).is_none() {
        return Err(ActionError::new(
            ActionErrorCode::OutOfRange,
            format!("{name} is too far to ask"),
        ));
    }
    let keys: Vec<FactKey> = inquiry
        .facts
        .iter()
        .filter_map(|fact| world.knowledge.key_of(fact))
        .filter(|key| knowledge::holds_key(world, target, *key).is_some())
        .collect();
    if keys.is_empty() {
        return Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            format!("{name} has nothing to tell you about that"),
        ));
    }
    let asked = format!(
        "{} asked you about \"{}\", and you told them what you know of it.",
        cap_first(&identify(them, &world.characters[player])),
        inquiry.title
    );

    let words = recount_all(world, target, &keys)
        .map_err(|reason| ActionError::new(ActionErrorCode::InvalidAction, reason))?;
    world
        .characters
        .get_mut(target)
        .expect("the target was found above")
        .notify_percept(asked);
    Ok(format!("{name} tells you: \"{}\"", words.join("\" and \"")))
}

/// The player's lodge control: hand accounts out of their notes to the
/// reviewer of one inquiry, to be read at the next ringing of the reviewer's
/// office.
///
/// Each of `picks` names an account by its fact and its speaker. It is copied
/// as it stands, so the review reads what was handed over and nothing its
/// speaker says afterwards. Everything is checked before anything is lodged: a
/// refusal names its own reason and leaves the world as it was. An account the
/// reviewer already has is passed over, and a bundle with nothing new in it is
/// refused.
///
/// A bundle already awaiting its review keeps its date. Lodging after a
/// finding sets a new date, and the whole bundle is read again.
pub fn lodge(
    world: &mut World,
    inquiry_id: &str,
    picks: &[(FactId, ActorId)],
) -> Result<String, ActionError> {
    let refuse = |code, message: &str| Err(ActionError::new(code, message));
    let Some(index) = world.inquiries.iter().position(|i| i.id == inquiry_id) else {
        return refuse(
            ActionErrorCode::InvalidAction,
            "nobody is looking into any such matter",
        );
    };
    let inquiry = &world.inquiries[index];
    if picks.is_empty() {
        return refuse(
            ActionErrorCode::InvalidArguments,
            "pick at least one account to lodge",
        );
    }
    let (Some(player), Some(them)) = (world.player_id(), world.characters.get(&inquiry.reviewer))
    else {
        return refuse(ActionErrorCode::UnknownTarget, "there is nobody to take it");
    };
    let reviewer = cap_first(&identify(&world.characters[player], them));
    if knowledge::player_in_earshot(world, &inquiry.reviewer).is_none() {
        return refuse(
            ActionErrorCode::OutOfRange,
            &format!("{reviewer} is not here to take it"),
        );
    }
    let mut new: Vec<Lodged> = Vec::new();
    for (fact, speaker) in picks {
        if !inquiry.facts.contains(fact) {
            return refuse(
                ActionErrorCode::InvalidAction,
                "that account is about another matter",
            );
        }
        let Some(account) = world
            .knowledge
            .player_learned
            .get(fact)
            .and_then(|receipt| receipt.accounts.iter().find(|a| a.speaker == *speaker))
        else {
            return refuse(
                ActionErrorCode::InvalidArguments,
                "there is no such account in your notes",
            );
        };
        let same = |lodged: &Lodged| lodged.fact == *fact && lodged.speaker == *speaker;
        if !inquiry.lodged.iter().chain(&new).any(same) {
            new.push(Lodged {
                fact: fact.clone(),
                speaker: speaker.clone(),
                origin: account.origin.clone(),
                view: account.view.clone(),
            });
        }
    }
    if new.is_empty() {
        return refuse(
            ActionErrorCode::InvalidAction,
            &format!("{reviewer} already has those"),
        );
    }
    let Some(time) = world.current_time else {
        return refuse(
            ActionErrorCode::InvalidAction,
            "there is no clock to set a review by",
        );
    };
    let taken = counted(new.len(), "account");
    let office = inquiry.review_office.label();
    let lodged = format!(
        "{} lodged {taken} with you about \"{}\". You will review what you have been given at \
         {office}.",
        cap_first(&identify(them, &world.characters[player])),
        inquiry.title
    );

    let inquiry = &mut world.inquiries[index];
    inquiry.lodged.extend(new);
    if inquiry.review_due.is_none() {
        inquiry.review_due = Some(time.next_ringing(inquiry.review_office));
    }
    world
        .characters
        .get_mut(&inquiry.reviewer)
        .expect("the reviewer was found above")
        .notify_percept(lodged);
    Ok(format!(
        "{reviewer} takes {taken}; the review is at {office}"
    ))
}

/// What a review held at `held` makes of an inquiry's bundle. It reads the
/// lodged copies and the inquiry's own rule and nothing else, so an account
/// still in the player's notes, or anything its speaker has heard since, has
/// no bearing on it.
///
/// A source is somebody who was there and whom a lodged account traces to: its
/// speaker, or the one person they had it from. Two people repeating one
/// witness are one source, and an account nobody can trace is none. Accounts
/// of one fact that tell it differently want settling by a first-hand one.
pub fn evaluate(inquiry: &Inquiry, held: WorldTime) -> Finding {
    let sources: BTreeSet<&ActorId> = inquiry
        .lodged
        .iter()
        .filter_map(|lodged| lodged.origin.as_ref())
        .collect();
    let unsettled = inquiry.facts.iter().any(|fact| {
        let of_fact: Vec<&Lodged> = inquiry
            .lodged
            .iter()
            .filter(|lodged| lodged.fact == *fact)
            .collect();
        let disagree = of_fact.iter().any(|lodged| lodged.view != of_fact[0].view);
        let first_hand = of_fact
            .iter()
            .any(|lodged| lodged.origin.as_ref() == Some(&lodged.speaker));
        disagree && !first_hand
    });
    let mut missing = Vec::new();
    if sources.len() < usize::from(inquiry.sources_needed) {
        missing.push(Support::IndependentSource);
    }
    if unsettled {
        missing.push(Support::SettlingWord);
    }
    let count = |n: usize| u8::try_from(n).unwrap_or(u8::MAX);
    Finding {
        held,
        accounts: count(inquiry.lodged.len()),
        sources: count(sources.len()),
        missing,
    }
}

/// Hold every review that has come due: read the bundle, record the finding
/// and tell the reviewer. Returns one diagnostic line for each review held.
///
/// The engine calls this on every poll. A review's date is a position on the
/// calendar, saved with its inquiry and cleared here, and that is the whole of
/// the once-only guard: neither a change of clock rate nor a restore can hold
/// a review twice or pass one by. Nobody has to be anywhere for it to happen.
pub fn hold_reviews(world: &mut World) -> Vec<String> {
    if world.inquiries.is_empty() {
        return Vec::new();
    }
    let Some(time) = world.current_time else {
        return Vec::new();
    };
    let now = time.game_days();
    let mut lines = Vec::new();
    for inquiry in &mut world.inquiries {
        if !inquiry.review_due.is_some_and(|due| due <= now) {
            continue;
        }
        let finding = evaluate(inquiry, time);
        let text = finding_text(inquiry, &finding);
        inquiry.review_due = None;
        inquiry.finding = Some(finding);
        lines.push(format!(
            "[inquiry] review of '{}' held at {}: {text}",
            inquiry.id,
            stamp(time.office, time.day)
        ));
        if let Some(reviewer) = world.characters.get_mut(&inquiry.reviewer) {
            reviewer.notify_percept(format!(
                "You have reviewed the accounts lodged with you about \"{}\". Your finding: \
                 {text}",
                inquiry.title
            ));
        }
    }
    lines
}

/// A finding in words: the inquiry's authored one when the bundle lacked
/// nothing, and otherwise what was read and which kinds of support are still
/// wanting. Both the player's record and the reviewer's own memory of the
/// review say it this way.
pub fn finding_text(inquiry: &Inquiry, finding: &Finding) -> String {
    if finding.missing.is_empty() {
        return inquiry.full_finding.clone();
    }
    let wanting: Vec<&str> = finding
        .missing
        .iter()
        .map(|support| support.wanting())
        .collect();
    format!(
        "Not established. {} read, from {}. Still wanting: {}.",
        counted(finding.accounts.into(), "account"),
        counted(finding.sources.into(), "source"),
        wanting.join("; ")
    )
}

/// A date as the city says it, for anything the player reads: "High Wick,
/// day 3".
pub(crate) fn stamp(office: Office, day: i64) -> String {
    format!("{}, day {day}", office.label())
}

/// "no sources", "1 account", "2 accounts".
fn counted(n: usize, noun: &str) -> String {
    match n {
        0 => format!("no {noun}s"),
        1 => format!("1 {noun}"),
        _ => format!("{n} {noun}s"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::character::{Character, CharacterSheet, Control, EconomicClass, Presence};
    use crate::clock::WorldClock;
    use crate::math::Vec3;

    fn actor(id: &str) -> ActorId {
        ActorId::from_raw(id)
    }

    fn body(id: &str, control: Control) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: actor(id),
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
            lore: None,
            presence: Presence::InCity,
            presence_epoch: 0,
            economic_class: EconomicClass::Resident,
        })
    }

    /// A hermetic world of four: two witnesses, a listener and a reviewer.
    fn world() -> World {
        let mut world = World::new();
        for id in ["ada", "bem", "cob", "rev"] {
            world.add_character(body(id, Control::Llm));
        }
        world
    }

    /// [`world`] with the pack in it and a player standing among the four.
    fn asked_world() -> World {
        let mut world = world();
        world.add_character(body("player", Control::Player));
        install(&mut world, &pack().to_string()).expect("the pack installs");
        world
    }

    fn step_away(world: &mut World, id: &str) {
        world
            .characters
            .get_mut(&actor(id))
            .expect("they are in the world")
            .state
            .position_m
            .x = crate::HEARING_RADIUS_M + 0.5;
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

    /// The sheet keeps three seats, so a matter seated fourth is one the
    /// speaker was never shown, and neither the row nor the verb may reach it.
    #[test]
    fn only_a_kept_seat_under_inquiry_in_earshot_is_recountable() {
        let mut world = asked_world();
        let cart = world
            .knowledge
            .key_of(&FactId::from_raw("test.cart"))
            .expect("the pack installed it");
        let other = |n: u32| FactKey(cart.0 + 1 + n);
        let ada = actor("ada");

        assert_eq!(
            recountable(&world, &ada, &[other(0), other(1), cart]),
            vec![cart]
        );
        assert!(recountable(&world, &ada, &[other(0), other(1), other(2), cart]).is_empty());

        step_away(&mut world, "ada");
        assert!(recountable(&world, &ada, &[cart]).is_empty());
    }

    /// The ask control records what the target holds, as they hold it, tells
    /// them they were asked, and reports what they told.
    #[test]
    fn asking_records_the_targets_account_and_tells_them_so() {
        let mut world = asked_world();
        let told = ask(&mut world, &actor("cob"), "test_cart").expect("cob holds it");
        let word = "the cart went over at the ford at some point with nobody on the box";
        assert_eq!(told, format!("A stranger (id cob) tells you: \"{word}\""));

        let accounts = &world.knowledge.player_learned[&FactId::from_raw("test.cart")].accounts;
        assert_eq!(accounts.len(), 1);
        assert_eq!(
            (
                accounts[0].speaker.as_str(),
                accounts[0].origin.as_ref().map(ActorId::as_str),
                accounts[0].hops,
                accounts[0].view.day_offset,
                accounts[0].word.as_str()
            ),
            ("cob", Some("ada"), 1, -1, word)
        );
        let asked = "A stranger (id player) asked you about \"The cart at the ford\", and you told \
                     them what you know of it.";
        assert_eq!(world.characters[&actor("cob")].inbox(), [asked]);
    }

    /// Each refusal has its own sentence and leaves the world as it was.
    #[test]
    fn a_refused_ask_says_why_and_changes_nothing() {
        let mut world = asked_world();
        step_away(&mut world, "bem");
        let before = world.clone();
        let mut refused = |target: &str, inquiry: &str| {
            let error = ask(&mut world, &actor(target), inquiry).expect_err("the ask is refused");
            (error.code, error.message)
        };

        assert_eq!(
            refused("ada", "test_wheel"),
            (
                ActionErrorCode::InvalidAction,
                "nobody is looking into any such matter".to_string()
            )
        );
        assert_eq!(
            refused("zz999", "test_cart"),
            (
                ActionErrorCode::UnknownTarget,
                "there is nobody by that id to ask".to_string()
            )
        );
        assert_eq!(
            refused("bem", "test_cart"),
            (
                ActionErrorCode::OutOfRange,
                "A stranger (id bem) is too far to ask".to_string()
            )
        );
        assert_eq!(
            refused("rev", "test_cart"),
            (
                ActionErrorCode::InvalidAction,
                "A stranger (id rev) has nothing to tell you about that".to_string()
            )
        );
        assert_eq!(world, before);
    }

    /// 07:00 on day 0, with a game day to the real hour.
    fn clock() -> WorldClock {
        WorldClock::new(3_600.0, Office::Dayspring, 0, 0.05)
    }

    /// A clocked world to lodge in: [`asked_world`]'s people, with `dan`, who
    /// also had the cart from `ada` and has the day right, and a second matter
    /// before the same reviewer.
    fn lodging_world() -> World {
        let mut world = world();
        world.add_character(body("dan", Control::Llm));
        world.add_character(body("player", Control::Player));
        let mut pack = pack();
        let mut add = |section: &str, row: Value| {
            pack[section].as_array_mut().expect("rows").push(row);
        };
        add(
            "facts",
            json!({
                "id": "test.wheel",
                "topic": "talk",
                "said": "the off wheel was cracked before the cart set out",
                "seeded": ["bem"],
                "decays": false
            }),
        );
        add(
            "retold",
            json!({"fact": "test.cart", "by": "dan", "from": "ada", "day_offset": 0}),
        );
        add(
            "inquiries",
            json!({
                "id": "test_wheel",
                "title": "The cracked wheel",
                "reviewer": "rev",
                "review_office": "high_wick",
                "facts": ["test.wheel"],
                "sources_needed": 1,
                "finding": "The wheel was cracked when it left the yard."
            }),
        );
        install(&mut world, &pack.to_string()).expect("the pack installs");
        world.current_time = Some(clock().at(0.0));
        world
    }

    /// The handle of `speaker`'s account of the cart.
    fn cart(speaker: &str) -> (FactId, ActorId) {
        (FactId::from_raw("test.cart"), actor(speaker))
    }

    /// Ask each of `speakers` about the cart, then lodge those accounts.
    fn lodge_cart(world: &mut World, speakers: &[&str]) -> String {
        let picks: Vec<(FactId, ActorId)> = speakers
            .iter()
            .map(|speaker| {
                ask(world, &actor(speaker), "test_cart").expect("they hold it");
                cart(speaker)
            })
            .collect();
        lodge(world, "test_cart", &picks).expect("the reviewer is here to take them")
    }

    /// What a review of the cart held this instant would find.
    fn reviewed(world: &World) -> Finding {
        evaluate(
            &world.inquiries[0],
            world.current_time.expect("a clocked world"),
        )
    }

    /// Acceptance 2, in full. The reviewer is told, the review is dated to the
    /// next ringing of their office, and the bundle lacks nothing.
    #[test]
    fn both_witnesses_lodged_get_the_authored_finding() {
        let mut world = lodging_world();
        let taken = lodge_cart(&mut world, &["ada", "bem"]);
        assert_eq!(
            taken,
            "A stranger (id rev) takes 2 accounts; the review is at High Wick"
        );
        assert_eq!(world.inquiries[0].review_due, Some(0.5));
        assert_eq!(world.inquiries[0].finding, None);
        let told = "A stranger (id player) lodged 2 accounts with you about \"The cart at the \
                    ford\". You will review what you have been given at High Wick.";
        assert_eq!(world.characters[&actor("rev")].inbox(), [told]);

        let finding = reviewed(&world);
        assert_eq!((finding.accounts, finding.sources), (2, 2));
        assert!(finding.missing.is_empty());
        assert_eq!(
            finding_text(&world.inquiries[0], &finding),
            "The cart went over by itself."
        );
    }

    /// Acceptance 2, in part. The review reads the bundle: the second witness
    /// has given an account too, but it was left in the notes, so it does not
    /// count and the finding names the one kind of support that is wanting.
    #[test]
    fn one_witness_lodged_wants_another_source_whatever_else_is_in_the_notes() {
        let mut world = lodging_world();
        ask(&mut world, &actor("bem"), "test_cart").expect("bem holds it");
        let taken = lodge_cart(&mut world, &["ada"]);
        assert_eq!(
            taken,
            "A stranger (id rev) takes 1 account; the review is at High Wick"
        );
        let notes = &world.knowledge.player_learned[&FactId::from_raw("test.cart")];
        assert_eq!(notes.accounts.len(), 2);

        let finding = reviewed(&world);
        assert_eq!((finding.accounts, finding.sources), (1, 1));
        assert_eq!(finding.missing, [Support::IndependentSource]);
        assert_eq!(
            finding_text(&world.inquiries[0], &finding),
            "Not established. 1 account read, from 1 source. Still wanting: a further account \
             that does not trace back to the same person."
        );
    }

    /// Acceptance 3 and the trap. Two people who both had it from one witness
    /// are one source, and as they tell the day differently with nobody who
    /// was there among them, their accounts want settling too. The witness's
    /// own account settles the day and is no new source; the second witness
    /// completes the bundle.
    #[test]
    fn two_retellings_of_one_witness_are_one_source_and_want_settling_where_they_disagree() {
        let mut world = lodging_world();
        lodge_cart(&mut world, &["cob", "dan"]);
        let finding = reviewed(&world);
        assert_eq!((finding.accounts, finding.sources), (2, 1));
        assert_eq!(
            finding.missing,
            [Support::IndependentSource, Support::SettlingWord]
        );
        assert_eq!(
            finding_text(&world.inquiries[0], &finding),
            "Not established. 2 accounts read, from 1 source. Still wanting: a further account \
             that does not trace back to the same person; the word of someone who was there, to \
             settle accounts that disagree."
        );

        lodge_cart(&mut world, &["ada"]);
        let finding = reviewed(&world);
        assert_eq!((finding.accounts, finding.sources), (3, 1));
        assert_eq!(finding.missing, [Support::IndependentSource]);

        lodge_cart(&mut world, &["bem"]);
        let finding = reviewed(&world);
        assert_eq!((finding.accounts, finding.sources), (4, 2));
        assert!(finding.missing.is_empty());
    }

    /// Each refusal has its own sentence, and one bad pick refuses the good
    /// ones beside it: nothing is lodged and no review is set.
    #[test]
    fn a_refused_lodging_says_why_and_lodges_nothing() {
        let mut world = lodging_world();
        ask(&mut world, &actor("ada"), "test_cart").expect("ada holds the cart");
        ask(&mut world, &actor("bem"), "test_wheel").expect("bem holds the wheel");
        let wheel = (FactId::from_raw("test.wheel"), actor("bem"));
        let refused = |world: &mut World, inquiry: &str, picks: &[(FactId, ActorId)]| {
            let before = world.clone();
            let error = lodge(world, inquiry, picks).expect_err("the lodging is refused");
            assert_eq!(*world, before, "a refused lodging leaves the world alone");
            (error.code, error.message)
        };

        assert_eq!(
            refused(&mut world, "test_axle", &[cart("ada")]),
            (
                ActionErrorCode::InvalidAction,
                "nobody is looking into any such matter".to_string()
            )
        );
        assert_eq!(
            refused(&mut world, "test_cart", &[]),
            (
                ActionErrorCode::InvalidArguments,
                "pick at least one account to lodge".to_string()
            )
        );
        assert_eq!(
            refused(&mut world, "test_cart", &[cart("ada"), wheel]),
            (
                ActionErrorCode::InvalidAction,
                "that account is about another matter".to_string()
            )
        );
        // `cob` holds the cart and was never asked, so there is no account.
        assert_eq!(
            refused(&mut world, "test_cart", &[cart("ada"), cart("cob")]),
            (
                ActionErrorCode::InvalidArguments,
                "there is no such account in your notes".to_string()
            )
        );

        step_away(&mut world, "rev");
        assert_eq!(
            refused(&mut world, "test_cart", &[cart("ada")]),
            (
                ActionErrorCode::OutOfRange,
                "A stranger (id rev) is not here to take it".to_string()
            )
        );
        world
            .characters
            .get_mut(&actor("rev"))
            .expect("the reviewer")
            .state
            .position_m
            .x = 0.0;

        let clock = world.current_time.take();
        assert_eq!(
            refused(&mut world, "test_cart", &[cart("ada")]),
            (
                ActionErrorCode::InvalidAction,
                "there is no clock to set a review by".to_string()
            )
        );
        world.current_time = clock;

        lodge(&mut world, "test_cart", &[cart("ada")]).expect("now it is taken");
        assert_eq!(
            refused(&mut world, "test_cart", &[cart("ada")]),
            (
                ActionErrorCode::InvalidAction,
                "A stranger (id rev) already has those".to_string()
            )
        );
    }

    /// Acceptance 7's guard, without the save. The review's date is a place on
    /// the calendar, so the T key changing the clock's rate on the way there
    /// neither loses it nor holds it twice, and once held it is not held again
    /// on any later day until something new is lodged.
    #[test]
    fn a_review_is_held_once_across_a_change_of_rate_and_again_only_after_a_new_lodging() {
        /// `polls` engine polls `step` real seconds apart: the clock is read
        /// onto the world, then the reviews are held.
        fn polled(
            world: &mut World,
            clock: WorldClock,
            now: &mut f64,
            step: f64,
            polls: usize,
        ) -> Vec<String> {
            let mut held = Vec::new();
            for _ in 0..polls {
                *now += step;
                world.current_time = Some(clock.at(*now));
                held.extend(hold_reviews(world));
            }
            held
        }
        let partial = "Not established. 1 account read, from 1 source. Still wanting: a further \
                       account that does not trace back to the same person.";

        let mut world = lodging_world();
        lodge_cart(&mut world, &["ada"]);
        let mut now = 0.0;

        // A game hour at the ordinary rate, to 08:00: not yet.
        let slow = clock();
        assert!(polled(&mut world, slow, &mut now, 1.0, 150).is_empty());
        assert_eq!(world.inquiries[0].finding, None);

        // Sixty times faster, through noon to 12:48.
        let fast = slow.with_scale(now, 60.0);
        assert_eq!(
            polled(&mut world, fast, &mut now, 0.05, 240),
            [format!(
                "[inquiry] review of 'test_cart' held at High Wick, day 0: {partial}"
            )]
        );
        let inquiry = &world.inquiries[0];
        assert_eq!(inquiry.review_due, None);
        let finding = inquiry.finding.as_ref().expect("the review was held");
        assert_eq!(
            (
                finding.held.day,
                finding.held.office,
                finding.missing.as_slice()
            ),
            (0, Office::HighWick, [Support::IndependentSource].as_slice())
        );
        assert_eq!(
            world.characters[&actor("rev")].inbox().last(),
            Some(&format!(
                "You have reviewed the accounts lodged with you about \"The cart at the ford\". \
                 Your finding: {partial}"
            ))
        );

        // On through the next day's noon, to 14:48 on day 1: nothing is due.
        assert!(polled(&mut world, fast, &mut now, 0.05, 1_300).is_empty());

        // The second witness is lodged, and the whole bundle is read again at
        // the next ringing, which is the day after's.
        lodge_cart(&mut world, &["bem"]);
        assert_eq!(world.inquiries[0].review_due, Some(2.5));
        let full = "[inquiry] review of 'test_cart' held at High Wick, day 2: The cart went over \
                    by itself.";
        assert_eq!(polled(&mut world, fast, &mut now, 0.05, 1_300), [full]);
        let finding = world.inquiries[0].finding.as_ref().expect("held again");
        assert_eq!((finding.accounts, finding.sources), (2, 2));
        assert!(finding.missing.is_empty());
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
