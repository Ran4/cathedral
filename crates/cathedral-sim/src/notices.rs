//! Ward notices (`features/law_and_order.md` M3): the gossip network the sim
//! already implies, made a world fact with a decay clock.
//!
//! A notice is a wrong on the ward's tongues — "an outland stranger took a
//! boy's spark at the tenter-frames" — raised by a law-cast actor with the
//! `raise_notice` verb after hearing a report or witnessing the deed. The
//! prose carries *descriptions and places, never ids* (the "unknown people"
//! rule; strangers have no names), while the record privately keeps the
//! accused/wronged ids the raiser supplied so a settlement can find its notice.
//!
//! **Settling is an act, not plumbing** (M3.5). A word comes off the ward's
//! tongues because somebody with standing — the law cast, or the wronged party
//! themselves — chose to end it with `settle_notice`. Handing over a purse
//! never settles anything by itself: it earns the acceptor a percept saying
//! this may be what the word wants, and the turn in which to judge it. That is
//! what lets a bench sergeant take a bribe and keep the word alive, and what
//! keeps a loaf bought from an accused baker from quietly laundering a theft.
//! Only two transfers settle on their own, because no verb could reach them:
//! the accused handing back *the very thing the notice names* ([`WardNotice::taken`]),
//! and the player as the wronged party, who has no verbs at all.
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
//!
//! TODO(M4, `features/law_and_order.md`): custody gives refusal a floor —
//! summons → warrant → seize → escort. The warrant deliberately comes from an
//! ignored summons rather than a bench: the lore's Civic Measure Court (three
//! rotating benchers, `lore/core_lore/secular_government.md`) is a whole second
//! system and is out of scope. If it is ever wanted, it slots in beside the
//! release paths — "committed to await a hearing" is the seam.

use std::{
    collections::BTreeSet,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    HEARING_RADIUS_M,
    attention::curiosity_of,
    character::{Character, Control},
    ids::{ActorId, ItemId},
    world::World,
};

/// The occupations that serve the city's law: they may `raise_notice` and
/// `settle_notice`, and they always carry the word.
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
    /// Who was wronged. They may settle their own word whether or not they
    /// serve the law, and they always carry it ([`carries`]) — the sheet is
    /// where they read the number `settle_notice` names.
    pub wronged: Option<ActorId>,
    /// The thing taken, when the raiser knew it (M3.5). The one transfer that
    /// needs no verb: the accused handing *this* back to the wronged is not a
    /// judgement call, so it settles the word on its own.
    pub taken: Option<ItemId>,
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
        taken: Option<ItemId>,
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
            taken,
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

    /// The notice with this id, while the ward is still saying it.
    pub fn get(&self, id: u64) -> Option<&WardNotice> {
        self.live.iter().find(|notice| notice.id == id)
    }

    /// Take one word off the ward's tongues. Per-notice by design: a settlement
    /// answers *a* wrong, never every wrong its subject is named in.
    pub fn settle(&mut self, id: u64) -> Option<WardNotice> {
        let index = self.live.iter().position(|notice| notice.id == id)?;
        Some(self.live.remove(index))
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

/// Whether the word reaches this person at all: the law always, and the
/// wronged party always — they are the one person who cannot need telling, and
/// since M3.5 the sheet is where they read the number `settle_notice` names.
/// Everyone else comes through a deterministic roll against their talkativeness
/// — the same "hash, never a fresh draw" idiom as `attention::opens_first`, so
/// which gossips repeat a story is reproducible in tests and drive scripts.
pub fn carries(world: &World, actor_id: &ActorId, notice_id: u64) -> bool {
    let Some(character) = world.characters.get(actor_id) else {
        return false;
    };
    if is_law(character) {
        return true;
    }
    if world
        .notices
        .get(notice_id)
        .is_some_and(|notice| notice.wronged.as_ref() == Some(actor_id))
    {
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

/// Who may end this word: the law cast, exactly as they may raise one — and
/// the wronged party named on it, whether or not they serve the law. It is the
/// boy's own spark; forgiving it is his to do.
pub fn may_settle(world: &World, actor_id: &ActorId, notice: &WardNotice) -> bool {
    notice.wronged.as_ref() == Some(actor_id)
        || world.characters.get(actor_id).is_some_and(is_law)
}

/// The live notices a transfer from `giver` to `acceptor` *might* be answering:
/// the giver is the accused, and the acceptor is the wronged party or any law
/// officer. It settles nothing — whether a crust answers a stolen spark is the
/// acceptor's judgement, and M3.5 exists to leave it there — but it is what
/// earns them the restitution percept and the turn to make it
/// (`actions::accept_offered_item`, `Engine::nudge_restitution_acceptor`).
pub fn restitution_candidates(world: &World, giver: &ActorId, acceptor: &ActorId) -> Vec<u64> {
    let acceptor_is_law = world.characters.get(acceptor).is_some_and(is_law);
    world
        .notices
        .live
        .iter()
        .filter(|notice| notice.accused.as_ref() == Some(giver))
        .filter(|notice| acceptor_is_law || notice.wronged.as_ref() == Some(acceptor))
        .map(|notice| notice.id)
        .collect()
}

/// The settlements no verb can reach, and the only transfers that still clear a
/// word by themselves (M3.5):
///
/// 1. the accused handing the wronged **the very thing the notice names** — not
///    a judgement call, so it needs no judge;
/// 2. the accused handing anything to the **player** as the wronged party — the
///    player has no verbs, so nothing else would ever end it.
///
/// Returns the settled notices so the caller can tell the carriers the word is
/// dead.
pub fn settle_on_return(
    world: &mut World,
    giver: &ActorId,
    acceptor: &ActorId,
    item_id: &ItemId,
) -> Vec<WardNotice> {
    let acceptor_is_player = world
        .characters
        .get(acceptor)
        .is_some_and(|character| character.control() == Control::Player);
    let mut settled = Vec::new();
    let live = std::mem::take(&mut world.notices.live);
    for notice in live {
        let answers = notice.accused.as_ref() == Some(giver)
            && notice.wronged.as_ref() == Some(acceptor)
            && (acceptor_is_player || notice.taken.as_ref() == Some(item_id));
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
            pockets: Vec::new(),
            frontbutt: None,
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

    /// M3.5: the wronged party carries their own word however taciturn they
    /// are. They cannot need telling — and since the verb names a notice by
    /// number, the sheet section is the only place they could read it off.
    #[test]
    fn the_wronged_carry_their_own_word_whatever_the_roll_says() {
        let taciturn = |id: &str| {
            let mut character = person(id, 0.0, Some("baker"));
            character.sheet.lore.as_mut().unwrap().curiosity = Some(0.0);
            character
        };
        let mut world = World::new();
        world.add_character(taciturn("wrngd"));
        world.add_character(taciturn("quiet"));
        let notice_id = world.notices.raise(
            "a stranger".into(),
            "a wrong".into(),
            None,
            None,
            None,
            ActorId::from_raw("srgnt"),
            None,
            Some(ActorId::from_raw("wrngd")),
            None,
        );

        assert!(carries(&world, &ActorId::from_raw("wrngd"), notice_id));
        assert!(
            !carries(&world, &ActorId::from_raw("quiet"), notice_id),
            "the ward's other silent bakers are spared it"
        );
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
