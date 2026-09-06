//! Where the city's own facts come from. Every mint is an explicit call at the
//! site that has the facts.
//!
//! `01_facts.md` says minting is "intercepted at `World::emit`" and takes `seeded`
//! from `DomainEvent::recipient_ids`. **Neither works, and the departure is
//! deliberate:**
//!
//! - [`World::emit`](crate::world::World::emit) is three lines that assign a
//!   sequence and push. It has no clock and no config, so it cannot stamp a game
//!   day or resolve a place.
//! - `recipient_ids` means something different per kind: carriers for the notice
//!   verbs (`actions::raise_notice` passes the curiosity-rolled `carriers`),
//!   hearers for the law verbs, and **empty** for `"commit"`, which is
//!   additionally gated `if gaol` and so never fires for a gate-arch commitment
//!   at all — the common case.
//!
//! So each mint computes its own earshot with
//! `World::characters_within(at, HEARING_RADIUS_M, None)` — `None` as `exclude`, so
//! the subject and the player are both in it, and distance-then-id order makes
//! `seeded` deterministic for free. The spec's promise that "who was there is
//! already answered correctly and is not re-implemented" is honoured by reusing the
//! radius and the scan, not the overloaded field. `actions::raise_ward_notice_for`
//! is the precedent: a coded write with no verb that computes its own earshot.

use std::collections::{BTreeMap, BTreeSet};

use crate::HEARING_RADIUS_M;
use crate::event::{DomainEvent, EventType};
use crate::knowledge::OCCASION_MIN_ASSERTION_CHARS;
use crate::lore::PlanningWard;

use crate::ids::{ActorId, AreaKey, FactId, FactKey};
use crate::knowledge::source::FactSource;
use crate::knowledge::{FACTS_MAX_LIVE, Fact, GarbleMask, Topic};
use crate::math::Vec3;
use crate::world::World;

/// One row of the whitelist. Stated **once**: the occasion gate's second limb (M4)
/// reads this same list, so "a percept that minted nothing" cannot drift out of
/// agreement with the mints across five call sites.
///
/// A coded mint needs no classifier — a seizure is `Law` because seizures are.
pub struct MintKind {
    /// The `DomainEvent::world_event` kind this covers.
    pub kind: &'static str,
    pub topic: Topic,
    pub garble: GarbleMask,
    /// The `said` template. `{subject}`, `{place}` and `{day}` are the only
    /// placeholders [`render_line`](crate::knowledge::render_line) substitutes, and
    /// every field this row may garble must appear in it or the swap would be
    /// invisible and the chain unwalkable — asserted by
    /// `every_mint_template_names_what_it_garbles`. A `{}`-braced name other than
    /// those three is substituted by the hook **before** install, so no fourth
    /// placeholder ever reaches a sheet.
    pub said: &'static str,
}

/// The custody commitment and `raise_notice`, both `Law`; the knell behind
/// `EngineCommand::Knell`; and the player's two stranger deeds.
///
/// **A large accepted sale is not used at all**: `inventory.rs` emits `"sale"` with
/// empty recipients, the price never reaches the event, and a mint inside
/// `market_sale`'s staged clone is silently discarded when the transaction errors —
/// the riskiest of the three candidate sites and the least anchored in the
/// schedule.
///
/// The first two rows prove the Law seam; they are **not** the band measurement. Two
/// `Law` facts cannot measure nine bands, which is what
/// [`plant_for_measurement`] is for.
pub const MINT_KINDS: &[MintKind] = &[
    MintKind {
        kind: "commit",
        topic: Topic::Law,
        garble: GarbleMask::ALL,
        said: "{subject} was taken in charge at {place} {day}, for {station}",
    },
    MintKind {
        kind: "raise_notice",
        topic: Topic::Law,
        garble: GarbleMask {
            subject: true,
            place: true,
            day: false,
        },
        said: "{subject} stands accused at {place}: {deed}",
    },
    MintKind {
        kind: "knell",
        topic: Topic::Blood,
        garble: GarbleMask {
            subject: false,
            place: false,
            day: true,
        },
        said: "a life of {years} years was counted out of Saint Maren's {day}",
    },
    MintKind {
        kind: "draw_mark",
        topic: Topic::Stranger,
        garble: GarbleMask {
            subject: false,
            place: true,
            day: true,
        },
        said: "{subject} chalked a mark on a door at {place} {day}",
    },
    MintKind {
        kind: "scrub_mark",
        topic: Topic::Stranger,
        garble: GarbleMask {
            subject: false,
            place: true,
            day: true,
        },
        said: "{subject} scrubbed a mark off a door at {place} {day}",
    },
];

/// These are the whitelist's only rows conditional on the actor's identity.
pub const STRANGER_DEED_KINDS: &[&str] = &["draw_mark", "scrub_mark"];

/// The Scold's legal Snuffing and civic summons both remind the city of law.
/// An exhaustive match makes another rope require an explicit meaning here.
pub fn peal_topic(rope: crate::engine::CivicRope) -> Option<Topic> {
    match rope {
        crate::engine::CivicRope::Curfew | crate::engine::CivicRope::Summons => Some(Topic::Law),
    }
}

/// The one installer: allocate the handles, freeze the salience inputs, resolve the
/// place, stamp the clock, seed everybody in earshot at hops 0, and put the telling
/// in the minting ward's air so it can be picked up.
///
/// A duplicate `id` is a **no-op**, not an error: the hooks build semantic ids
/// (`law.taken.<prisoner>`, `law.word.<notice>`), so a second commitment of the
/// same person is the same proposition and mints nothing new.
///
/// `own` is empty for every coded mint — a coded mint has no first-person telling
/// to give anybody, including the subject, which means `render_line` returns `None`
/// for them and the prisoner is never told about their own arrest in the third
/// person: the self-subject rule falling out of the data. M4's `mint_claim` passes
/// the speaker's own words and nothing else differs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn install_fact(
    world: &mut World,
    id: FactId,
    topic: Topic,
    said: String,
    own: BTreeMap<ActorId, String>,
    subject: Vec<ActorId>,
    seeded: BTreeSet<ActorId>,
    at: Vec3,
    garble: GarbleMask,
    decays: bool,
    source: FactSource,
    game_days: Option<f64>,
) -> Option<FactKey> {
    if !world.knowledge_enabled {
        return None;
    }
    let (handle, sequence) = world.knowledge.next_handles();
    // The one function that computes the two frozen salience inputs — the authored
    // path and both coded mints must not disagree about a household.
    let (quiet_among, craft_ear) = crate::knowledge::frozen_ears(world, &subject);
    let place = area_key_at(world, at);
    let day = world.current_time.map(|time| time.day);
    let key = world.knowledge.install(Fact {
        id,
        key: handle,
        sequence,
        subject,
        place,
        day,
        said,
        own,
        seeded,
        garble,
        decays,
        topic,
        minted_game_days: game_days,
        quiet_among,
        craft_ear,
        source,
    })?;
    // Fresh news is at heat 1.0 in the air of the ward it happened in, at hops 0,
    // with no `via` — nobody told the ward, it saw. A standing fact
    // (`decays: false`) is **not** put in the air: it is answerable, never loud.
    if decays && let Some(ward) = world.ward_at(at) {
        world
            .knowledge
            .seed_air(ward, key, 1.0, game_days.unwrap_or(0.0));
    }
    Some(key)
}

/// The named area this point stands in, as a dense key. `None` in every world with
/// an empty `AreaMap`. One function, because [`install_fact`] and M4's collision
/// test must agree about "the place".
pub(crate) fn area_key_at(world: &World, at: Vec3) -> Option<AreaKey> {
    world
        .area_map
        .containing_area(at)
        .map(|area| area.id.clone())
        .and_then(|id| world.area_map.key_of_id(&id))
}

/// [`install_fact`] with `seeded` computed from earshot and no `own` line — the
/// ordinary coded mint, and the only kind gameplay can reach.
#[allow(clippy::too_many_arguments)]
pub fn mint(
    world: &mut World,
    id: FactId,
    topic: Topic,
    said: String,
    subject: Vec<ActorId>,
    at: Vec3,
    garble: GarbleMask,
    decays: bool,
    source: FactSource,
    game_days: Option<f64>,
) -> Option<FactKey> {
    let seeded = world
        .characters_within(at, crate::HEARING_RADIUS_M, None)
        .into_iter()
        .collect();
    let key = install_fact(
        world,
        id,
        topic,
        said,
        BTreeMap::new(),
        subject,
        seeded,
        at,
        garble,
        decays,
        source,
        game_days,
    )?;
    // A witnessed event is learned too. Keep the receipt write in `learn`:
    // its first-hand refusal protects the witness while recording the telling.
    if let Some(player) = world.player_id().cloned()
        && world
            .knowledge
            .fact(key)
            .is_some_and(|f| f.seeded.contains(&player))
    {
        crate::knowledge::learn(
            world,
            &player,
            key,
            crate::knowledge::Telling {
                hops: 0,
                from: None,
                heat: 1.0,
                view: Default::default(),
            },
            game_days,
        );
    }
    Some(key)
}

/// How many mouths the cadence pack is seeded to — `02_numbers.md` §4's **K = 4**,
/// which every closed form at both ends of the band takes as given.
///
/// A **measurement** constant, so it lives here beside
/// [`plant_for_measurement`] and never in the sim's public constants block, where
/// it would read as a number about the model.
#[doc(hidden)]
pub const PACK_MOUTHS: usize = 4;

/// Plant one measurement fact with an **authored** seeded set.
///
/// The band's instrument, never shipped content and unreachable from any verb. It
/// exists because [`mint`] computes earshot itself, and at the Wickmarket
/// `HEARING_RADIUS_M` is **one mouth** (measured: 1 body inside 20 m, 7 inside
/// 40 m) — a pack whose K is whatever happens to be standing there cannot reproduce
/// `02_numbers.md` §4, whose every closed form takes K = 4.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn plant_for_measurement(
    world: &mut World,
    id: FactId,
    topic: Topic,
    said: String,
    subject: Vec<ActorId>,
    seeded: BTreeSet<ActorId>,
    at: Vec3,
    garble: GarbleMask,
    game_days: Option<f64>,
) -> Option<FactKey> {
    install_fact(
        world,
        id,
        topic,
        said,
        BTreeMap::new(),
        subject,
        seeded,
        at,
        garble,
        true,
        FactSource::authored(),
        game_days,
    )
}

/// Fill `live` to [`FACTS_MAX_LIVE`] with **standing** filler rows nobody holds
/// and nothing volunteers, so a cost guard's O(live) walk in `poll_person`'s
/// deposit phase runs at the length it is bounded at and not at the eleven rows
/// two authored facts and a pack make (`02_numbers.md` §6 conditions the
/// `seeded_by_actor` decision on that walk's saturated cost). `decays: false`, so
/// they enter no ward's air and change no roll: they cost the walk and nothing
/// else. Returns how many were planted. `--pollen-saturate` and the 20,000 guard.
#[doc(hidden)]
pub fn fill_live_for_measurement(world: &mut World, at: Vec3, game_days: Option<f64>) -> usize {
    let mut planted = 0;
    for index in 0..FACTS_MAX_LIVE {
        if world.knowledge.len() >= FACTS_MAX_LIVE {
            break;
        }
        let key = install_fact(
            world,
            FactId::from_raw(format!("pollen.fill.{index:03}")),
            Topic::Talk,
            "a filler matter nobody holds".to_string(),
            BTreeMap::new(),
            Vec::new(),
            BTreeSet::new(),
            at,
            GarbleMask::NONE,
            false,
            FactSource::authored(),
            game_days,
        );
        if key.is_none() {
            break;
        }
        planted += 1;
    }
    planted
}

/// The arrest, as the ward will repeat it. Topic `Law`.
///
/// Called from `Engine::announce_commitment`, immediately above the
/// `confiscate_the_taking` call — **not** from `World::emit`: the `"commit"` event
/// is `if gaol`-gated with empty `recipient_ids`, so it misses every gate-arch
/// commitment, and `emit` itself has no clock. Earshot is computed here.
///
/// Sourced on the custody record, so it dies on release ([`FactSource::custody`]) —
/// carriers simply stop saying it, with no `forget` verb and no LLM cooperation.
/// The pollen stir sweep invalidates it before the next hearsay reading.
pub fn mint_commitment(
    world: &mut World,
    prisoner: &ActorId,
    officer: Option<&ActorId>,
    station: &str,
    at: Vec3,
    game_days: Option<f64>,
) -> Option<FactKey> {
    let row = MINT_KINDS.iter().find(|row| row.kind == "commit")?;
    let _ = officer; // the officer is the mouth, not the subject
    mint(
        world,
        FactId::from_raw(format!("law.taken.{}", prisoner.as_str())),
        row.topic,
        row.said.replace("{station}", station),
        vec![prisoner.clone()],
        at,
        row.garble,
        true,
        FactSource::custody(prisoner.clone()),
        game_days,
    )
}

/// The notice's own words, as hearsay. Topic `Law`.
///
/// **`None` when the notice names nobody the sim can resolve.** A subject-less
/// `Law` fact cannot garble its subject, and M5's hearsay rung — a garbled subject
/// producing a wrongful summons the player can watch get raised, and settle — is
/// the reason this mint exists. A notice that is only prose stays only prose;
/// `notices::carries` already carries it to every mouth that matters.
///
/// Sourced `FactSource::event("raise_notice", notice_id)`, which never
/// invalidates — deliberately: a rumour outliving the notice it came from is the
/// spec's own legitimate case ("the ward goes on saying a thing that is no longer
/// so until it cools").
pub fn mint_from_notice(
    world: &mut World,
    notice_id: u64,
    raiser: &ActorId,
    game_days: Option<f64>,
) -> Option<FactKey> {
    let row = MINT_KINDS.iter().find(|row| row.kind == "raise_notice")?;
    let notice = world.notices.get(notice_id)?;
    let accused = notice.accused.clone()?;
    let said = row.said.replace("{deed}", &notice.deed);
    let at = world.characters.get(raiser)?.position_m();
    mint(
        world,
        FactId::from_raw(format!("law.word.{notice_id}")),
        row.topic,
        said,
        vec![accused],
        at,
        row.garble,
        true,
        FactSource::event("raise_notice", notice_id as i64),
        game_days,
    )
}

/// A life counted out of Saint Maren's, with no person named: one bell is not
/// a name. Only the day garbles. Equal ages on the same game day deliberately
/// collapse to one proposition, `knell.<day>.<years>`.
pub fn mint_knell(world: &mut World, at: Vec3, years: u32, game_days: f64) -> Option<FactKey> {
    let row = MINT_KINDS.iter().find(|row| row.kind == "knell")?;
    mint(
        world,
        FactId::from_raw(format!("knell.{}.{years}", game_days.floor() as i64)),
        row.topic,
        row.said.replace("{years}", &years.to_string()),
        Vec::new(),
        at,
        row.garble,
        true,
        FactSource::event("knell", world.event_sequence),
        Some(game_days),
    )
}

/// The player's hand on the city's walls, in a fixed template, never the
/// event's free text. An authored hand doing the same work is not a stranger.
///
/// The journal records the player's own witnessed deed. Far wards greet them
/// through the NPC's sheet; the player never deposits news about themselves.
/// Getting their own account there first means speaking, not a self-subject
/// exception to `may_carry`.
pub fn mint_stranger_deed(
    world: &mut World,
    event: &DomainEvent,
    player_id: &ActorId,
    game_days: f64,
) -> Option<FactKey> {
    if !world.knowledge_enabled
        || event.event_type != EventType::WorldEvent
        || event.actor_id.as_ref() != Some(player_id)
        || !world.is_player(player_id)
        || !STRANGER_DEED_KINDS.contains(&event.kind.as_str())
    {
        return None;
    }
    let at = event.position_m?;
    let row = MINT_KINDS.iter().find(|row| row.kind == event.kind)?;
    let key = mint(
        world,
        FactId::from_raw(format!("stranger.{}.{}", event.kind, event.sequence)),
        row.topic,
        row.said.to_string(),
        vec![player_id.clone()],
        at,
        row.garble,
        true,
        FactSource::event(&event.kind, event.sequence),
        Some(game_days),
    )?;
    if world
        .knowledge
        .fact(key)
        .is_some_and(|f| f.seeded.contains(player_id))
    {
        super::record_player_deed(world, player_id, key, Some(game_days));
    }
    Some(key)
}

/// The occasion gate's first limb: somebody asserted a thing to this hearer
/// that they do not hold, which is the only condition that puts `raise_word` on
/// a sheet by speech.
///
/// The 24-byte / no-`'?'` test is a **pre-filter, not the gate** — it only skips
/// "Aye." and "What of it?". The gate is a `holds()` lookup: for every live fact
/// whose subject this line names, does the hearer already have it? If any of
/// them is held, the gate stays shut, because repetition needs no verb
/// (`01_facts.md`, "One verb, and why repetition does not get one").
///
/// It fires for the player's novel lie precisely because a novel claim names
/// nobody the store has a fact about, so the lookup is vacuously true.
/// Bounded by `named.len()` (≤ ~30: the hearer's `knows` plus everyone in
/// earshot) times their ≤ `HOLDINGS_MAX` holdings, on one `say`.
pub fn note_assertion(
    world: &mut World,
    speaker: &ActorId,
    hearer: &ActorId,
    text: &str,
    game_days: f64,
) {
    if !world.knowledge_enabled {
        return;
    }
    if text.contains('?') || text.len() < OCCASION_MIN_ASSERTION_CHARS {
        return;
    }
    if !world
        .characters
        .get(hearer)
        .is_some_and(|h| h.control().is_llm())
    {
        return;
    }

    let named = crate::actions::subjects_named(world, hearer, Some(speaker), text);
    // When the line names nobody the hearer could place, the thing asserted is
    // about the speaker — D34's own fallback, and what makes the player's novel
    // lie vacuously unheld.
    let about: Vec<ActorId> = if named.is_empty() {
        vec![speaker.clone()]
    } else {
        named.clone()
    };
    let already = world.knowledge.facts().any(|(key, fact)| {
        fact.subject.iter().any(|subject| about.contains(subject))
            && crate::knowledge::holds_key(world, hearer, key).is_some()
    });
    if already {
        return;
    }
    world.knowledge.note_occasion(
        hearer,
        named.first().cloned(),
        Some(speaker.clone()),
        game_days,
    );
}

/// The occasion gate's second limb: they saw something the whitelist does not
/// cover, so a word about it is theirs to raise.
///
/// Restricted to `EventType::WorldEvent` (`event.rs:14-22`) — speech goes
/// through limb 1, item traffic is not news, and sounds and gestures are not
/// events *about the world*. Reads `MINT_KINDS` and nothing else, so "a percept
/// that minted nothing" cannot drift out of agreement with the mints as the
/// whitelist grows (D33).
pub fn note_unminted_event(world: &mut World, event: &DomainEvent, game_days: f64) {
    if !world.knowledge_enabled {
        return;
    }
    // Belt and braces: the one caller already matched this arm, but a second
    // caller must not be able to arm an occasion off a gesture.
    if event.event_type != EventType::WorldEvent {
        return;
    }
    // `DomainEvent::kind` is a plain `String` (`event.rs:30`), not an `Option`.
    if MINT_KINDS.iter().any(|row| row.kind == event.kind) {
        return;
    }
    let Some(at) = event.position_m else { return };
    for hearer in world.characters_within(at, HEARING_RADIUS_M, None) {
        if world.characters[&hearer].control().is_llm() {
            world
                .knowledge
                .note_occasion(&hearer, None, None, game_days);
        }
    }
}

/// The **only** constructor for a claimed fact, so no guardrail can be bypassed
/// by writing a `Fact` literal at a call site. Everything an LLM can reach goes
/// through here, and so does the drive-mode poke.
///
/// Unforgeable, in order. `source` is `FactSource::claimed(speaker)` and is not
/// a parameter — a model can mint claims; it can never mint truths. `seeded` is
/// the speaker and nobody else: a claim cannot seat knowledge in other people's
/// heads. `decays` is always true: nothing said aloud becomes a standing fact of
/// the world. The speaker gets an `own` line — the claim *is* in their own
/// words, and without one their own sheet would tell a liar they merely heard
/// it. **And a claim is a template like every other fact (D58)**: the resolved
/// subject's display name in `said` becomes `{subject}`, so a reader who does
/// not know that person reads the role and not the name (the toy's "with your
/// name filed off"), and the mask is the topic's default narrowed to what the
/// sentence can actually garble — `subject` only if the substitution happened,
/// never `place` or `day`, which a free-text claim does not carry. A claim
/// cannot seal itself against drift, including in the mouth of the person who
/// made it up.
///
/// `from` is the mouth that armed them, so `chain()` walks back past the raiser
/// to whoever put the words in their head — the player, in the case this exists
/// for.
pub fn mint_claim(
    world: &mut World,
    speaker: &ActorId,
    topic: Topic,
    said: String,
    subject: Vec<ActorId>,
    from: Option<ActorId>,
    game_days: Option<f64>,
) -> Option<FactKey> {
    if !world.knowledge_enabled {
        return None;
    }
    let at = world.characters.get(speaker)?.position_m();
    // The template: the first resolved subject's name → `{subject}`, word
    // boundary and case-insensitive (`actions::replace_name` — the inverse of
    // `text_mentions_name`'s matcher, `actions.rs:289`, and one function with it).
    let (template, subject_named) = match subject.first() {
        Some(who) => match world.characters.get(who).map(|c| c.name().to_string()) {
            Some(name) => crate::actions::replace_name(&said, &name, "{subject}"),
            None => (said.clone(), false),
        },
        None => (said.clone(), false),
    };
    let garble = GarbleMask {
        subject: GarbleMask::default_for(topic).subject && subject_named,
        place: false,
        day: false,
    };
    let own = BTreeMap::from([(speaker.clone(), said)]); // the words they actually said
    let seeded = BTreeSet::from([speaker.clone()]);
    let key = install_fact(
        world,
        claim_id(world),
        topic,
        template,
        own,
        subject,
        seeded,
        at,
        garble,
        true,
        FactSource::claimed(speaker.clone()),
        game_days,
    )?;
    world.knowledge.seat_claimant(speaker, key, from, game_days);
    if let Some(ward) = world.ward_at(at)
        && let Some(row) = world.knowledge.air_mut().get_mut(&(ward, key))
    {
        row.via = Some(speaker.clone());
    }
    Some(key)
}

fn claim_id(world: &World) -> FactId {
    FactId::from_raw(format!("claim.{}", world.knowledge.next_sequence))
}

/// Structural identity in the raiser's ward, never a comparison of prose.
pub fn collides_in_air(
    world: &World,
    ward: PlanningWard,
    topic: Topic,
    subject: Option<&ActorId>,
    place: Option<AreaKey>,
    day: Option<i64>,
) -> bool {
    world.knowledge.ward_air(ward).any(|(key, _)| {
        world.knowledge.fact(key).is_some_and(|f| {
            f.topic == topic && f.subject.first() == subject && f.place == place && f.day == day
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{Character, CharacterSheet, Control};
    use crate::clock::{Office, Weekday, WorldTime};

    fn actor(id: &str) -> ActorId {
        ActorId::from_raw(id)
    }

    fn character(id: &str, name: &str) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: actor(id),
            name: name.to_string(),
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
            presence: crate::character::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::character::EconomicClass::Resident,
        })
    }

    /// A world with a clock, an officer and a prisoner standing together, so a
    /// coded mint has an earshot and a day to stamp.
    fn law_world() -> World {
        let mut world = World::new();
        world.current_time = Some(WorldTime {
            day: 3,
            fraction: 0.25,
            office: Office::Dayspring,
            weekday: Weekday::Bellday,
        });
        world.add_character(character("prisnr", "Grigor Ashe"));
        world.add_character(character("bailff", "Ede Clove"));
        world
    }

    /// The mask and the template are one statement, not two: a row that may garble
    /// a field the template never names would swap it invisibly, and the chain
    /// would be unwalkable.
    ///
    /// The second half is the closed list — every `{…}` in a `said` is either one of
    /// the three the renderer substitutes, or a name the hook fills in *before*
    /// install, so no fourth placeholder can reach a sheet.
    #[test]
    fn every_mint_template_names_what_it_garbles() {
        const HOOK_FILLED: [&str; 3] = ["station", "deed", "years"];
        for row in MINT_KINDS {
            for (field, garbled) in [
                ("subject", row.garble.subject),
                ("place", row.garble.place),
                ("day", row.garble.day),
            ] {
                if garbled {
                    assert!(
                        row.said.contains(&format!("{{{field}}}")),
                        "{}: the mask garbles {field} and the template never names it, \
                         so the swap would be invisible: {}",
                        row.kind,
                        row.said
                    );
                }
            }
            let mut rest = row.said;
            while let Some(open) = rest.find('{') {
                let tail = &rest[open + 1..];
                let close = tail.find('}').unwrap_or_else(|| {
                    panic!("{}: an unclosed placeholder in {}", row.kind, row.said)
                });
                let name = &tail[..close];
                assert!(
                    ["subject", "place", "day"].contains(&name) || HOOK_FILLED.contains(&name),
                    "{}: {{{name}}} is neither a placeholder `render_line` substitutes \
                     nor one a hook fills before install",
                    row.kind
                );
                rest = &tail[close + 1..];
            }
        }
    }

    /// The whitelist is one table, so M4's second occasion limb ("a percept that
    /// minted nothing") reads the same list the mints do, and M5 adding a row is a
    /// deliberate edit rather than a drift between five call sites.
    #[test]
    fn the_whitelist_is_stated_once() {
        assert_eq!(
            MINT_KINDS.len(),
            5,
            "the five coded kinds and the occasion exclusion must be one list"
        );
        for row in &MINT_KINDS[..2] {
            assert_eq!(
                row.topic,
                Topic::Law,
                "{} is not Law; two Law facts cannot measure nine bands, which is \
                 why the cadence pack exists",
                row.kind
            );
        }
        // Every kind is the one its hook looks up by name.
        let kinds: Vec<&str> = MINT_KINDS.iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            vec!["commit", "raise_notice", "knell", "draw_mark", "scrub_mark"]
        );
    }

    /// A second commitment of the same person is the same proposition: the id is
    /// semantic, so the store refuses it and the city does not start saying it
    /// twice.
    #[test]
    fn a_duplicate_mint_is_a_no_op() {
        let mut world = law_world();
        let at = Vec3::new(0.0, 0.91, 0.0);
        let first = mint_commitment(
            &mut world,
            &actor("prisnr"),
            Some(&actor("bailff")),
            "The Stone House",
            at,
            Some(3.25),
        );
        assert!(first.is_some());
        assert_eq!(world.knowledge.len(), 1);
        assert_eq!(
            mint_commitment(
                &mut world,
                &actor("prisnr"),
                Some(&actor("bailff")),
                "The Stone House",
                at,
                Some(3.30),
            ),
            None,
            "the same arrest is the same fact"
        );
        assert_eq!(world.knowledge.len(), 1);

        // What the one mint actually put there: the station in the template, both
        // bodies in earshot seeded, the day stamped off the world's clock, and no
        // `own` line — so the prisoner is never told about their own arrest in the
        // third person.
        let fact = world
            .knowledge
            .fact_by_id(&FactId::from_raw("law.taken.prisnr"))
            .expect("the row");
        assert!(fact.said.ends_with("for The Stone House"), "{}", fact.said);
        assert!(fact.said.contains("{subject}") && fact.said.contains("{place}"));
        assert_eq!(
            fact.seeded,
            BTreeSet::from([actor("prisnr"), actor("bailff")])
        );
        assert_eq!(fact.subject, vec![actor("prisnr")]);
        assert_eq!(fact.day, Some(3));
        assert_eq!(fact.minted_game_days, Some(3.25));
        assert!(fact.own.is_empty());
        assert!(fact.decays);
        // No `AreaMap` in a hermetic world, so there is no named place to resolve.
        // The **ward** is a different question and it does resolve: the grid is
        // baked from the embedded `homes.json`, not from this world, so a body
        // standing on real city ground is in a real ward wherever the test set it
        // down. So fresh news is in that ward's air at heat 1.0, hops 0, and with
        // no `via` — nobody told the ward, it saw.
        assert_eq!(fact.place, None);
        let key = fact.key;
        let ward = world
            .ward_at(at)
            .expect("the origin stands in the shipped city");
        let air = world
            .knowledge
            .drift(ward, key)
            .expect("the air row")
            .clone();
        assert_eq!(crate::knowledge::heat_pct(air.heat), 100);
        assert_eq!((air.hops, air.via), (0, None));
        assert_eq!(world.knowledge.air_entries(), 1);
    }

    /// A notice that names nobody the sim can resolve mints nothing: a subject-less
    /// `Law` fact cannot garble its subject, which is the whole reason this mint
    /// exists.
    #[test]
    fn a_notice_that_names_nobody_mints_nothing() {
        let mut world = law_world();
        let raiser = actor("bailff");
        let anonymous = world
            .notices
            .raise(
                "a man in a red hood".into(),
                "short measure at the meal stall".into(),
                Some("the Wickmarket".into()),
                Some("this morning".into()),
                Some(3.25),
                raiser.clone(),
                None,
                None,
                None,
            )
            .expect("the ward has room");
        assert_eq!(
            mint_from_notice(&mut world, anonymous, &raiser, Some(3.25)),
            None
        );
        assert!(world.knowledge.is_empty());

        // Named, and it mints — with the notice's own deed inside the fixed
        // template, so what the fact asserts is that the accusation exists.
        let named = world
            .notices
            .raise(
                "Grigor Ashe".into(),
                "short measure at the meal stall".into(),
                Some("the Wickmarket".into()),
                Some("this morning".into()),
                Some(3.25),
                raiser.clone(),
                Some(actor("prisnr")),
                None,
                None,
            )
            .expect("the ward has room");
        let key = mint_from_notice(&mut world, named, &raiser, Some(3.25)).expect("it mints");
        let fact = world.knowledge.fact(key).expect("the row");
        assert_eq!(fact.id, FactId::from_raw(format!("law.word.{named}")));
        assert_eq!(fact.subject, vec![actor("prisnr")]);
        assert_eq!(
            fact.said,
            "{subject} stands accused at {place}: short measure at the meal stall"
        );
        assert!(
            !fact.garble.day,
            "a notice carries its own date, not a drifting one"
        );
        // An unknown notice number is nothing to mint, and never a panic.
        assert_eq!(
            mint_from_notice(&mut world, 9_999, &raiser, Some(3.25)),
            None
        );
    }

    /// Same age and same day is the same proposition; another age is new news.
    #[test]
    fn knells_of_the_same_age_and_day_coalesce() {
        let mut world = law_world();
        assert!(mint_knell(&mut world, Vec3::new(0.0, 0.91, 0.0), 17, 3.25).is_some());
        assert!(mint_knell(&mut world, Vec3::new(0.0, 0.91, 0.0), 17, 3.25).is_none());
        assert!(mint_knell(&mut world, Vec3::new(0.0, 0.91, 0.0), 18, 3.25).is_some());
        assert_eq!(world.knowledge.len(), 2);
    }
}
