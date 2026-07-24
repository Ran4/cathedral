//! Ward notices (`features/law_and_order.md` M3): the gossip network the sim
//! already implies, made a world fact with a decay clock.
//!
//! A notice is a wrong on the ward's tongues — "an outland stranger took a
//! boy's spark at the tenter-frames" — raised by a law-cast actor with the
//! `raise_notice` verb after hearing a report or witnessing the deed. The
//! prose carries *descriptions and places, never ids* (the "unknown people"
//! rule; strangers have no names), while the record privately keeps the
//! accused/wronged ids the raiser supplied so restitution can settle it: the
//! accused handing the taken thing back to the wronged — or paying any law
//! officer — clears the word through the existing offer/accept machinery
//! (`actions::accept_offered_item`).
//!
//! Who carries the word: the law cast always; ordinary citizens are diluted
//! through the same deterministic-hash idiom as the curiosity gate
//! (`attention::curiosity_of` — the sim contains no RNG anywhere, so which
//! gossips repeat a story is a pure function of the notice and the person).
//! Carriers see the standing `word_in_the_ward` sheet section and get one
//! arrival percept — the percept is load-bearing, not decoration: a non-empty
//! inbox is what admits an idle turn (`attention::Novelty`), and the sheet
//! section alone would be invisible to the news gate.
//!
//! Confrontation is proximity, not procedure: when a law-cast carrier first
//! shares hearing range with the accused while the notice is live, they get a
//! face-to-face percept (`confront`, ticked by the engine) and act as their
//! character. No accuse verb exists — speech plus the percept is enough.

use std::{
    collections::BTreeSet,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    HEARING_RADIUS_M,
    attention::curiosity_of,
    character::Character,
    ids::ActorId,
    world::World,
};

/// The occupations that serve the city's law: they may `raise_notice`, they
/// always carry the word, and a fine paid to any of them settles a notice.
/// Deliberately narrower than `attention::RESERVED_TRADES` — a scribe is
/// reserved company, not a law officer — and deliberately including the
/// toll-house's revenue men, who share Odo Trask's counter at the Tallage.
pub const LAW_OCCUPATIONS: &[&str] = &[
    "bailiff_and_gaoler",
    "court_officer",
    "civic_officer",
    "custody_clerk",
    "watchman_and_keeper",
    "militia_and_soldier",
    "revenue_worker",
];

/// How long the ward talks before the word dies down on its own: twenty game
/// days (at the shipped clock, twenty real hours) — a small wrong is worth
/// most of a season of cold shoulders, and the only quick way out is
/// restitution. Still finite, so an unsettled wrong does not brand a stranger
/// forever.
pub const NOTICE_LIFE_GAME_DAYS: f64 = 20.0;

/// The most live notices the ward holds at once: raising past the cap drops
/// the oldest. Guards the prompt budget (the sheet renders at most
/// [`NOTICES_SHEET_MAX`] anyway) and bounds an LLM that spams the verb.
pub const NOTICES_MAX_LIVE: usize = 8;

/// The most notices one sheet renders — newest first, like `recent_history`.
pub const NOTICES_SHEET_MAX: usize = 4;

/// One wrong on the ward's tongues.
#[derive(Debug, Clone, PartialEq)]
pub struct WardNotice {
    /// Monotonic per-world; the salt that decides which gossips carry it.
    pub id: u64,
    /// The accused as prose — a description, never a bare id.
    pub about: String,
    /// What they did.
    pub deed: String,
    /// Where, as prose ("the tenter-frames"), when the raiser gave one.
    pub place: Option<String>,
    /// "Highmarket's Dayspring" — stamped from the world clock at raise, so
    /// the word ages in speech the way the sheet's `the_day` reads. `None` in
    /// clock-less worlds.
    pub since: Option<String>,
    /// Absolute game-days at raise; `None` in clock-less worlds, where the
    /// notice never decays (the tests' hermetic worlds have no time to pass).
    pub raised_game_days: Option<f64>,
    pub raised_by: ActorId,
    /// Private settlement linkage, never rendered: who the raiser meant.
    /// Without it the notice can only expire.
    pub accused: Option<ActorId>,
    /// Who was wronged — restitution is the accused handing them the taking.
    pub wronged: Option<ActorId>,
    /// Law-cast carriers already served the face-to-face percept, once each.
    served: BTreeSet<ActorId>,
}

impl WardNotice {
    /// The one prose line everything renders — the sheet bullet, and the body
    /// of the arrival/confrontation/settlement percepts.
    pub fn line(&self) -> String {
        let mut line = format!("{} — {}", self.about, self.deed);
        if let Some(place) = &self.place {
            line.push_str(&format!(", at {place}"));
        }
        if let Some(since) = &self.since {
            line.push_str(&format!("; the word since {since}"));
        }
        line
    }
}

/// The ward's live notices. A field of [`World`] — the prompt renders them and
/// actions branch on them, so they are world state, not engine bookkeeping —
/// but the decay clock is driven from outside (`Engine::poll`), because the
/// sim is clock-free by design.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notices {
    live: Vec<WardNotice>,
    next_id: u64,
}

impl Notices {
    /// Record a raised notice, oldest-out past [`NOTICES_MAX_LIVE`]. Returns
    /// the assigned id.
    #[allow(clippy::too_many_arguments)]
    pub fn raise(
        &mut self,
        about: String,
        deed: String,
        place: Option<String>,
        since: Option<String>,
        raised_game_days: Option<f64>,
        raised_by: ActorId,
        accused: Option<ActorId>,
        wronged: Option<ActorId>,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.live.push(WardNotice {
            id,
            about,
            deed,
            place,
            since,
            raised_game_days,
            raised_by,
            accused,
            wronged,
            served: BTreeSet::new(),
        });
        if self.live.len() > NOTICES_MAX_LIVE {
            self.live.remove(0);
        }
        id
    }

    pub fn live(&self) -> &[WardNotice] {
        &self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// Drop every notice whose life has run out. Undated notices never decay.
    pub fn expire(&mut self, game_days: f64) {
        self.live.retain(|notice| {
            notice
                .raised_game_days
                .is_none_or(|raised| game_days - raised < NOTICE_LIFE_GAME_DAYS)
        });
    }
}

/// Whether this character serves the city's law (by lore occupation).
pub fn is_law(character: &Character) -> bool {
    character
        .lore()
        .and_then(|profile| profile.occupation_id.as_deref())
        .is_some_and(|occupation| LAW_OCCUPATIONS.contains(&occupation))
}

/// Whether the word reaches this person at all: the law always; everyone else
/// through a deterministic roll against their talkativeness — the same
/// "hash, never a fresh draw" idiom as `attention::opens_first`, so which
/// gossips repeat a story is reproducible in tests and drive scripts.
pub fn carries(world: &World, actor_id: &ActorId, notice_id: u64) -> bool {
    let Some(character) = world.characters.get(actor_id) else {
        return false;
    };
    if is_law(character) {
        return true;
    }
    let mut hasher = DefaultHasher::new();
    "ward_notice_carry".hash(&mut hasher);
    actor_id.hash(&mut hasher);
    notice_id.hash(&mut hasher);
    let roll = (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64;
    roll < curiosity_of(world, actor_id)
}

/// The carrier ids for a notice, in roster order, minus `except`. LLM actors
/// only: the player cannot carry gossip — they experience the word from the
/// outside, as the city cooling toward them.
pub fn carrier_ids(world: &World, notice_id: u64, except: &ActorId) -> Vec<ActorId> {
    world
        .roster
        .iter()
        .filter(|id| {
            *id != except
                && world
                    .characters
                    .get(*id)
                    .is_some_and(|character| character.control().is_llm())
                && carries(world, id, notice_id)
        })
        .cloned()
        .collect()
}

/// Settle every live notice this transfer answers: the accused (`giver`)
/// handing the taking back to the wronged, or paying a law officer. Returns
/// the settled notices so the caller can tell the carriers the word is dead.
pub fn settle_on_transfer(world: &mut World, giver: &ActorId, acceptor: &ActorId) -> Vec<WardNotice> {
    let acceptor_is_law = world.characters.get(acceptor).is_some_and(is_law);
    let mut settled = Vec::new();
    let live = std::mem::take(&mut world.notices.live);
    for notice in live {
        let names_giver = notice.accused.as_ref() == Some(giver);
        let answers = names_giver
            && (acceptor_is_law || notice.wronged.as_ref() == Some(acceptor));
        if answers {
            settled.push(notice);
        } else {
            world.notices.live.push(notice);
        }
    }
    settled
}

/// The face-to-face percept: when a law-cast carrier is within hearing of the
/// accused while the word is live, they are told — once per (notice, officer)
/// pair — and their now non-empty inbox admits the idle turn the confrontation
/// needs. Ticked by the engine every poll; costs nothing while no notice
/// names a present accused.
pub fn confront(world: &mut World) {
    if world.notices.live.is_empty() {
        return;
    }
    // (notice index, officer, line) triples collected first: the mutation
    // below needs the world's characters, and the served-set the notices.
    let mut due: Vec<(usize, ActorId, String)> = Vec::new();
    for (index, notice) in world.notices.live.iter().enumerate() {
        let Some(accused) = &notice.accused else {
            continue;
        };
        let Some(accused_character) = world.characters.get(accused) else {
            continue;
        };
        if !world.is_present(accused) {
            continue;
        }
        let position = accused_character.position_m();
        for officer_id in world.characters_within(position, HEARING_RADIUS_M, Some(accused)) {
            if notice.served.contains(&officer_id) {
                continue;
            }
            if !world.characters.get(&officer_id).is_some_and(is_law) {
                continue;
            }
            due.push((
                index,
                officer_id,
                format!(
                    "the one the ward's word names is within reach of you right now: {}",
                    notice.line()
                ),
            ));
        }
    }
    for (index, officer_id, line) in due {
        if let Some(officer) = world.characters.get_mut(&officer_id) {
            officer.notify_percept(line);
        }
        world.notices.live[index].served.insert(officer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GOAL_NONE, Significance,
        character::{Character, CharacterSheet, Control},
        lore::{LoreProfile, PlanningWard},
        math::Vec3,
    };
    use std::collections::BTreeSet;

    fn person(id: &str, x: f64, occupation: Option<&str>) -> Character {
        let lore = occupation.map(|occupation_id| LoreProfile {
            significance: Significance::Ambient,
            planning_ward: PlanningWard::Fabric,
            age: 30,
            gender: "m".into(),
            occupation_id: Some(occupation_id.into()),
            occupation_display: None,
            title: None,
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "Fabric".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: None,
            core_character_description: String::new(),
            extended_character_description: String::new(),
            curiosity: None,
        });
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore,
            presence: crate::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        })
    }

    fn raise(notices: &mut Notices, raised_game_days: Option<f64>, accused: Option<&str>) -> u64 {
        notices.raise(
            "a stranger".into(),
            "a wrong".into(),
            None,
            None,
            raised_game_days,
            ActorId::from_raw("srgnt"),
            accused.map(ActorId::from_raw),
            None,
        )
    }

    #[test]
    fn notices_decay_on_the_game_clock_and_undated_ones_never_do() {
        let mut notices = Notices::default();
        raise(&mut notices, Some(10.0), None);
        raise(&mut notices, None, None);

        notices.expire(10.0 + NOTICE_LIFE_GAME_DAYS - 0.1);
        assert_eq!(notices.live().len(), 2, "still inside its life");
        notices.expire(10.0 + NOTICE_LIFE_GAME_DAYS);
        assert_eq!(notices.live().len(), 1, "the dated word has died down");
        notices.expire(1000.0);
        assert_eq!(notices.live().len(), 1, "an undated notice never decays");
    }

    #[test]
    fn the_ward_holds_a_bounded_number_of_notices_oldest_out() {
        let mut notices = Notices::default();
        for _ in 0..(NOTICES_MAX_LIVE + 1) {
            raise(&mut notices, None, None);
        }
        assert_eq!(notices.live().len(), NOTICES_MAX_LIVE);
        assert_eq!(notices.live()[0].id, 2, "the oldest was dropped");
    }

    /// The face-to-face percept: a law officer within hearing of the accused
    /// is told once — not again on the next poll, and never when out of range.
    #[test]
    fn confront_serves_each_officer_once_within_hearing() {
        let mut world = World::new();
        world.add_character(person("thief", 0.0, None));
        world.add_character(person("srgnt", 10.0, Some("bailiff_and_gaoler")));
        world.add_character(person("baker", 10.0, Some("baker")));
        world.add_character(person("faroff", 30.0, Some("bailiff_and_gaoler")));
        raise(&mut world.notices, None, Some("thief"));

        confront(&mut world);
        let inbox_of = |world: &World, id: &str| {
            world.characters[&ActorId::from_raw(id)].inbox().to_vec()
        };
        assert_eq!(
            inbox_of(&world, "srgnt"),
            ["the one the ward's word names is within reach of you right now: a stranger — a wrong"]
        );
        assert!(inbox_of(&world, "baker").is_empty(), "not law, not told");
        assert!(inbox_of(&world, "faroff").is_empty(), "out of hearing");

        // The next poll adds nothing; a settled inbox stays settled.
        confront(&mut world);
        assert_eq!(inbox_of(&world, "srgnt").len(), 1);
    }
}
