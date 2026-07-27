//! Custody (`features/law_and_order.md` M4): the floor under refusal.
//!
//! **Custody is a state; the grab is only its enforcement.** `seize` does not
//! take hold of anybody — it puts them in charge of an officer and names a
//! destination, and they are free to walk. Compliance is a 100–200 m walk beside
//! an LLM sergeant who talks the whole way and can let go at any point: *the
//! escort is the content, not the cell.* The hand comes onto the arm only when
//! the arrangement is broken.
//!
//! ```text
//! Free ──seize──► In charge ──grab──► Held ──arrive──► Committed ──release──► Free
//!                     │                  │                  │
//!                     └──── release ─────┴──────────────────┘
//! ```
//!
//! The player and the cast run the same diagram, and only the player half is
//! expensive. The sim is already the authoritative mover for NPCs, so an NPC in
//! charge is simply *slaved to their escort* ([`follow_escorts`]) — no clamp, no
//! host message, no `controller.rs`, and both parties walk at
//! [`crate::WALK_SPEED_MPS`], so there is no speed disparity to engineer around.
//! Everything hard about M4 — the 4.4× faster player, the tether, the host-side
//! reflex, the strain meter, the dead-man timer — exists because the host owns
//! the player's feet and the sim does not.
//!
//! **Confinement is a person, not a door.** A prisoner goes to the *nearest*
//! station ([`Stations`]) — a gate arch, the toll-house counter, the Bellstand
//! watch tower — because Wickmarket → River Gate is three minutes pinned at
//! 1.8 m/s and the nearest posting is fifty. Most of those are not lockable
//! rooms, so the keeper holds the threshold and walking out is an escape
//! attempt. Only the Stone House (M5) has a real door.
//!
//! [`Confinement::Committed`] is one flag with two histories: the eight
//! `prisoner`-circumstance characters the lore already holds on Stone House
//! rations arrive in it at seed time, and tonight's arrest arrives in it at the
//! end of an escort. That they are indistinguishable afterwards is the sign the
//! design is right.
//!
//! TODO(the bench): a court would slot in beside the release paths — "committed
//! to await a hearing" is exactly the seam. M4 deliberately leaves it out: three
//! benchers sitting as the Civic Measure Court
//! (`lore/core_lore/secular_government.md`) is a whole second system, so here
//! the warrant comes from an ignored summons instead.

use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    character::StatusKind,
    ids::{ActorId, PlaceId},
    math::Vec3,
    places::PlaceRegistry,
    world::World,
};

/// Arm's length. The same order as [`crate::WELL_ARRIVE_RADIUS_M`] and inside
/// the 4 m offer radius: a grab is a reflex at contact range, never a lasso.
/// An LLM turn costs seconds and the player runs at 12 m/s, so anything wider
/// would be decided about a player who is already thirty metres gone.
///
/// Measured in three dimensions, like the offer radius — not planar. Everyone
/// walks the same ground ([`crate::nav::WALK_Y`], and the player spawns on it),
/// so on the street the height term is zero and the distance is simply the
/// stride. Where it is *not* zero it is doing real work: somebody on an
/// overhead bridge is out of arm's reach, which is correct, and the city has
/// plenty of those. (A test that stands the two parties at different heights is
/// therefore testing a boundary it did not mean to.)
pub const CUSTODY_REACH_M: f64 = 3.0;

/// How far someone merely *in charge* may drift before the officer closes.
/// Deliberately [`crate::SOCIAL_PULL_RADIUS_M`]: enough to step aside, look at
/// a stall, talk to somebody you pass — not enough to leave by accident.
pub const CUSTODY_LEASH_M: f64 = 8.0;

/// The leash again, measured from the station instead of from an officer: how
/// far a **committed** prisoner may be from the place they are kept before
/// walking away is walking out (M5d).
///
/// Deliberately [`CUSTODY_LEASH_M`] and not [`STATION_ARRIVE_RADIUS_M`]: the
/// Stone House's room is about 9.6 × 6.6 m inside, so somebody standing in the
/// far corner is already better than five metres from its place node, and a
/// four-metre tether would make sitting against the back wall an escape.
/// Confinement is a person at a threshold, not a chalk circle.
pub const COMMITTED_ROAM_M: f64 = CUSTODY_LEASH_M;

/// Where the escort walks the prisoner: close enough to be normally within
/// [`CUSTODY_REACH_M`] without being held.
pub const CUSTODY_ESCORT_CONTACT_M: f64 = 1.5;

/// How far a held body may move around the grip point. Turn, face them, face
/// away, circle — you simply cannot leave.
pub const CUSTODY_TETHER_M: f64 = 1.5;

/// Close enough to the station to be committed.
pub const STATION_ARRIVE_RADIUS_M: f64 = 4.0;

/// The most people the law may hold at once, city-wide, excluding the authored
/// inmates. Nothing else in the sim removes a person from the world and the
/// economy is made of named individuals — gaol the wrong baker and the bread
/// round stops — so a sergeant having a bad day must not be able to empty the
/// Wickmarket. Small on purpose: M5's cell is already at the stage's six-actor
/// limit with its authored eight, so arrivals are counted in ones and twos.
pub const CUSTODY_MAX_ARRESTS: usize = 4;

/// The station cap, in **real** seconds: four minutes, no exceptions, whatever
/// the models do. At `seconds_per_day: 3600` an office bell can be 8.5 real
/// minutes off, which is far too long to stare at a gate arch with nobody to
/// talk to but the keeper.
pub const STATION_HOLD_SECONDS: f64 = 240.0;

/// The Stone House's own ceiling (M5), six minutes against the station's four.
/// Deliberately different: the station cap is the price of standing at a gate
/// arch, and this one is sized for a room worth staying in — eight inmates who
/// have nothing to do but talk to you. If they ever want to be one number, it
/// is because the Stone House failed at being interesting, and that is the
/// thing to fix instead.
pub const STONE_HOUSE_HOLD_SECONDS: f64 = 360.0;

/// The posted gaol fee, in sparks. The lore is explicit that *inventing* a fee
/// is extortion, so it is a fixed number the keeper states identically to every
/// asker (a pilgrim badge was one spark).
pub const GAOL_FEE_SPARKS: u32 = 3;

/// If the holder takes no turn for this long, the hold releases itself. A
/// player must never be pinned by a provider outage, a killed process, or plain
/// lane starvation — with one LLM turn in flight across the whole cast, a busy
/// scene can starve a holder past a minute with nothing broken at all.
/// Releasing then is correct, not a false positive: the officer can always take
/// hold again.
pub const CUSTODY_DEAD_MAN_SECONDS: f64 = 60.0;

/// The places a prisoner can be taken, by the display name the place registry
/// knows them under. **M4 needs no new geometry**: every one of these is an
/// existing posting, and the list is M2's argument repeated — law presence where
/// people are, not one building.
///
/// The Stone House is deliberately *not* first and never a default: it is for
/// grave matters, and until M5 builds it the name simply resolves to nothing.
pub const STATION_PLACE_NAMES: &[&str] = &[
    "Tallage toll-house",
    "Bellstand watch-bell tower",
    "The Stone Gate",
    "The Wool Gate",
    "The Harne Gate",
    "The River Gate",
    "The Reed Postern",
    "The Stone House",
];

/// The one station where the waiting is the content rather than the price of
/// it, and the only place with a real door (M5).
pub const STONE_HOUSE_PLACE_NAME: &str = "The Stone House";

/// Where a prisoner is being taken, resolved once at `seize` so the escort and
/// the HUD read the same answer even if the registry is later reloaded.
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub place_id: PlaceId,
    /// How people speak of it — what the HUD line and the officer's `say` name.
    pub name: String,
    pub point: Vec3,
    /// The Stone House holds longer ([`STONE_HOUSE_HOLD_SECONDS`]).
    pub stone_house: bool,
}

impl Station {
    /// The real-time ceiling on being kept here, whatever the models do.
    pub fn hold_seconds(&self) -> f64 {
        if self.stone_house {
            STONE_HOUSE_HOLD_SECONDS
        } else {
            STATION_HOLD_SECONDS
        }
    }
}

/// Which box of the diagram somebody is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confinement {
    /// Taken and walking to the station beside the officer. Free to step aside,
    /// talk to somebody, look at a stall — within [`CUSTODY_LEASH_M`].
    InCharge,
    /// Arrived. Held by a person at a threshold, not by a lock.
    Committed,
}

/// One person in the law's hands.
#[derive(Debug, Clone, PartialEq)]
pub struct CustodyRecord {
    /// Who took them and walks them there. `None` for the authored inmates, who
    /// were already inside when the run began and have no arresting officer.
    pub officer: Option<ActorId>,
    /// Everyone with a hand on their arm right now — **refcounted**, because two
    /// officers letting go one at a time must not free anybody. Empty means
    /// merely in charge, which is the pleasant path and the usual one.
    pub holders: Vec<ActorId>,
    /// The word this answers. `None` for the authored inmates.
    pub notice_id: Option<u64>,
    pub station: Station,
    pub state: Confinement,
    /// Whether the city was already holding them when the run began. They do
    /// not count against [`CUSTODY_MAX_ARRESTS`] — the cap exists to stop a
    /// bad-tempered sergeant emptying the Wickmarket, not to evict the lore.
    pub authored: bool,
    /// The arrangement is broken and the officer is coming to take hold: set
    /// when the separation passes [`CUSTODY_LEASH_M`], cleared when a hand
    /// actually lands.
    ///
    /// It has to be a **latch**, not a distance test, and that is the whole
    /// subtlety of the grab. [`CUSTODY_REACH_M`] is 3 m and the leash is 8, so
    /// "at 3 m while outside the leash" describes no reachable position at all;
    /// what the design means is *at arm's reach, having broken the leash* — a
    /// state that begins when you stray and ends when they have you. Without the
    /// latch the reflex can only fire on somebody actively running, and a player
    /// who walks off and then stands still while a sergeant crosses the square
    /// is never taken, which is exactly the case the approach-is-the-tell design
    /// is built around.
    pub closing: bool,
    /// The bell they were told they go at (M5c), stamped **once** at commitment
    /// so the stated sentence cannot drift between two readings of it — the
    /// keeper says a thing out loud and the city has to keep saying the same
    /// thing. `None` while merely in charge, and for a station: at a gate arch
    /// the honest answer is "when the keeper says", and it is the Stone House
    /// where the waiting is the content rather than the price of it.
    ///
    /// Constant for the life of the record on purpose — the player's custody
    /// rides a hot channel that republishes on change, so nothing time-varying
    /// may live beside it or the message would be new on every poll.
    pub sentence_office: Option<crate::clock::Office>,
    /// Absolute game-days at which that bell rings, or `None` in a clock-less
    /// world — where, exactly as an undated summons never comes due, the
    /// sentence never runs out and the real-time ceiling is the only door.
    pub sentence_due_game_days: Option<f64>,
    /// Real seconds at `seize`, and at commitment. The sim has no clock by
    /// design, so — exactly as `WardNotice::raised_game_days` does — these are
    /// stamped by the engine and read back by it.
    pub seized_at: f64,
    pub committed_at: Option<f64>,
    /// When the escort last took a turn, for the dead-man timer. Refreshed by
    /// the engine every time the officer thinks.
    pub officer_last_turn: f64,
}

impl CustodyRecord {
    pub fn is_held(&self) -> bool {
        !self.holders.is_empty()
    }
}

/// Everyone the law is holding. A field of [`crate::world::World`] — the prompt
/// renders it, the behaviour ladder is guarded by it and actions branch on it,
/// so it is world state — but every real-time clock in it is driven from
/// outside, because the sim is clock-free by design.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Custody {
    held: BTreeMap<ActorId, CustodyRecord>,
}

impl Custody {
    pub fn get(&self, prisoner: &ActorId) -> Option<&CustodyRecord> {
        self.held.get(prisoner)
    }

    pub fn get_mut(&mut self, prisoner: &ActorId) -> Option<&mut CustodyRecord> {
        self.held.get_mut(prisoner)
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Whether the law has this person at all — in charge or committed. The one
    /// question the behaviour ladder and `go_to` ask.
    pub fn holds(&self, prisoner: &ActorId) -> bool {
        self.held.contains_key(prisoner)
    }

    /// Whether somebody has a hand on their arm right now.
    pub fn is_held(&self, prisoner: &ActorId) -> bool {
        self.held
            .get(prisoner)
            .is_some_and(CustodyRecord::is_held)
    }

    /// Whether they are confined at a station or in the Stone House. The state
    /// the eight authored inmates are seeded into and an arrest arrives in.
    pub fn is_confined(&self, prisoner: &ActorId) -> bool {
        self.held
            .get(prisoner)
            .is_some_and(|record| record.state == Confinement::Committed)
    }

    /// Whether this person is walking somebody to a station right now — the
    /// officer of, or a hand on, a record still merely in charge. The question
    /// the behaviour ladder's escort guard asks: an escort whose own needs
    /// divert them walks off the leash and frees the prisoner behind them.
    /// Deliberately false once the prisoner is committed — the keeper at the
    /// threshold is free to go and eat.
    pub fn is_escorting(&self, escort: &ActorId) -> bool {
        self.held.values().any(|record| {
            record.state == Confinement::InCharge
                && (record.officer.as_ref() == Some(escort) || record.holders.contains(escort))
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ActorId, &CustodyRecord)> {
        self.held.iter()
    }

    /// How many arrests count against [`CUSTODY_MAX_ARRESTS`] — the authored
    /// inmates never do.
    pub fn arrest_count(&self) -> usize {
        self.held.values().filter(|record| !record.authored).count()
    }

    /// Whether the law has room to take one more person.
    pub fn has_room(&self) -> bool {
        self.arrest_count() < CUSTODY_MAX_ARRESTS
    }

    /// Take someone in charge. Replaces any existing record — a second officer
    /// seizing an already-taken prisoner is a handover, not a second custody.
    pub fn seize(
        &mut self,
        prisoner: ActorId,
        officer: ActorId,
        notice_id: Option<u64>,
        station: Station,
        now: f64,
    ) {
        self.held.insert(
            prisoner,
            CustodyRecord {
                officer: Some(officer),
                holders: Vec::new(),
                notice_id,
                station,
                state: Confinement::InCharge,
                authored: false,
                closing: false,
                sentence_office: None,
                sentence_due_game_days: None,
                seized_at: now,
                committed_at: None,
                officer_last_turn: now,
            },
        );
    }

    /// Seed one of the eight the city was already holding (M5b). They arrive
    /// committed, with no officer, no notice, and no claim on the cap.
    pub fn seed_inmate(&mut self, prisoner: ActorId, station: Station) {
        self.held.insert(
            prisoner,
            CustodyRecord {
                officer: None,
                holders: Vec::new(),
                notice_id: None,
                station,
                state: Confinement::Committed,
                authored: true,
                closing: false,
                sentence_office: None,
                sentence_due_game_days: None,
                seized_at: 0.0,
                committed_at: Some(0.0),
                officer_last_turn: 0.0,
            },
        );
    }

    /// Put a hand on the arm. Idempotent per holder, and refcounted.
    ///
    /// A hand landing ends the closing: they strayed, the officer came, and the
    /// chase such as it was is over.
    pub fn grab(&mut self, prisoner: &ActorId, holder: ActorId) -> bool {
        let Some(record) = self.held.get_mut(prisoner) else {
            return false;
        };
        if !record.holders.contains(&holder) {
            record.holders.push(holder);
        }
        record.closing = false;
        true
    }

    /// One holder lets go. The custody itself survives — two officers, one lets
    /// go, you are still held (by the other) and still in charge either way.
    pub fn let_go(&mut self, prisoner: &ActorId, holder: &ActorId) {
        if let Some(record) = self.held.get_mut(prisoner) {
            record.holders.retain(|id| id != holder);
        }
    }

    /// Every hand comes off, but the custody stands — what a broken grip and
    /// the dead-man timer both do.
    pub fn release_grip(&mut self, prisoner: &ActorId) {
        if let Some(record) = self.held.get_mut(prisoner) {
            record.holders.clear();
        }
    }

    /// Arrived: the escort ends and the keeper's threshold begins.
    pub fn commit(&mut self, prisoner: &ActorId, now: f64) -> bool {
        let Some(record) = self.held.get_mut(prisoner) else {
            return false;
        };
        if record.state == Confinement::Committed {
            return false;
        }
        record.state = Confinement::Committed;
        record.committed_at = Some(now);
        record.holders.clear();
        true
    }

    /// Let them go entirely. Returns the record — which still names whoever had
    /// hold — so the caller can say what ended and to whom. A hold that ends in
    /// silence looks exactly like one that did not.
    pub fn release(&mut self, prisoner: &ActorId) -> Option<CustodyRecord> {
        self.held.remove(prisoner)
    }

    /// Whoever this person may let go of: those they took, and those they have
    /// hold of. Roster order.
    pub fn prisoners_of(&self, officer: &ActorId) -> Vec<ActorId> {
        self.held
            .iter()
            .filter(|(_, record)| {
                record.officer.as_ref() == Some(officer) || record.holders.contains(officer)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Everyone a departed character was holding or being held by, tidied. Called
    /// when the round takes somebody out of the city.
    ///
    /// Returns the freed with their records — which still name whoever else had
    /// hold — so the caller can say what ended and to whom, exactly as
    /// [`Self::release`] does: a hold that ends in silence looks exactly like
    /// one that did not. [`forget_departed`] is that saying, for the callers
    /// that hold a whole [`World`].
    pub fn forget(&mut self, gone: &ActorId) -> Vec<(ActorId, CustodyRecord)> {
        self.held.remove(gone);
        let mut freed = Vec::new();
        for (prisoner, record) in &mut self.held {
            record.holders.retain(|id| id != gone);
            if record.officer.as_ref() == Some(gone) && record.state == Confinement::InCharge {
                freed.push(prisoner.clone());
            }
        }
        // An escort who left the city cannot walk anybody anywhere; their
        // prisoners are simply free, which is the same answer the dead-man timer
        // gives for the same reason.
        freed
            .into_iter()
            .map(|prisoner| {
                let record = self
                    .held
                    .remove(&prisoner)
                    .expect("the freed came from the map");
                (prisoner, record)
            })
            .collect()
    }
}

/// The station picker: the **nearest** posting to where the seizure happened,
/// never the Stone House by default.
///
/// This is M2's argument repeated. One law site for an 840×700 m city means a
/// three-minute march at 1.8 m/s; eight postings mean fifty to a hundred and
/// fifty metres, and the escort stays a scene rather than a commute. The Stone
/// House is excluded here on purpose — it is where a grave matter is committed,
/// which is a decision, not a distance.
pub fn nearest_station(places: &PlaceRegistry, from: Vec3) -> Option<Station> {
    let mut best: Option<(Station, f64)> = None;
    for name in STATION_PLACE_NAMES {
        if *name == STONE_HOUSE_PLACE_NAME {
            continue;
        }
        let Some(entry) = places.named(name) else {
            continue;
        };
        let distance = f64::hypot(entry.point.x - from.x, entry.point.z - from.z);
        if best.as_ref().is_none_or(|(_, best)| distance < *best) {
            best = Some((
                Station {
                    place_id: entry.id.clone(),
                    name: entry.name.clone(),
                    point: entry.point,
                    stone_house: false,
                },
                distance,
            ));
        }
    }
    best.map(|(station, _)| station)
}

/// The Stone House itself, when it has been built (M5). `None` until then,
/// which is exactly why [`nearest_station`] never falls back to it.
pub fn stone_house(places: &PlaceRegistry) -> Option<Station> {
    places.named(STONE_HOUSE_PLACE_NAME).map(|entry| Station {
        place_id: entry.id.clone(),
        name: entry.name.clone(),
        point: entry.point,
        stone_house: true,
    })
}

// --------------------------------------------------------------- the escort

/// Walk the escorted: every NPC merely in charge is placed at the officer's
/// shoulder, and anyone who reaches their station is committed. Returns the ids
/// that moved, so the engine can put them on the hot movement channel.
///
/// **This is the whole of NPC custody, and it is why M4b′ costs almost
/// nothing.** The sim is already the authoritative mover for the cast, so a
/// prisoner is simply slaved to the person walking them: no clamp, no host
/// message, no `controller.rs`, and both parties move at
/// [`crate::WALK_SPEED_MPS`], so there is no 3.8× disparity to engineer around.
/// The player's case is the expensive one and none of it applies here.
///
/// The player is never moved by this — the sim has no authority over their feet
/// — but they *are* committed by it when they walk to the station themselves,
/// which is the compliance path working exactly as it reads.
pub fn follow_escorts(world: &mut World, now: f64) -> EscortStep {
    if world.custody.is_empty() {
        return EscortStep::default();
    }
    let player_id = world.player_id().cloned();
    let mut moved = Vec::new();
    let mut arrived = Vec::new();

    let walking: Vec<(ActorId, Option<ActorId>, Vec3, bool)> = world
        .custody
        .iter()
        .filter(|(_, record)| record.state == Confinement::InCharge)
        .map(|(prisoner, record)| {
            (
                prisoner.clone(),
                record.officer.clone(),
                record.station.point,
                Some(prisoner) == player_id.as_ref(),
            )
        })
        .collect();

    for (prisoner, officer, station_point, is_player) in walking {
        // Where the officer stands, and which way they are facing.
        let anchor = officer.as_ref().and_then(|officer| {
            world
                .characters
                .get(officer)
                .map(|officer| (officer.position_m(), officer.facing_yaw()))
        });
        if !is_player && let Some((officer_position, yaw)) = anchor {
            // A pace behind the shoulder, on the officer's own heading: yaw 0
            // looks toward -Z, which is the world's own convention.
            let behind = Vec3::new(
                officer_position.x + yaw.sin() * CUSTODY_ESCORT_CONTACT_M,
                officer_position.y,
                officer_position.z + yaw.cos() * CUSTODY_ESCORT_CONTACT_M,
            );
            if let Some(character) = world.characters.get_mut(&prisoner) {
                if character.position_m() != behind {
                    character.state.position_m = behind;
                    character.state.facing_yaw = yaw;
                    moved.push(prisoner.clone());
                }
                // A led body has no walk of its own; the mover must not fight
                // the escort for the same position.
                character.state.movement = None;
            }
        }
        let at = world
            .characters
            .get(&prisoner)
            .map(|character| character.position_m());
        if at.is_some_and(|at| {
            f64::hypot(station_point.x - at.x, station_point.z - at.z) <= STATION_ARRIVE_RADIUS_M
        }) {
            arrived.push(prisoner);
        }
    }

    arrived.retain(|prisoner| world.custody.commit(prisoner, now));

    // **Little else in the sim knows about custody, and what does is a guard,
    // not a mover.** `round::decide`'s rung 0 and the `go_to` refusal stop a
    // held body *choosing* to walk, and `round`'s `set_route` — the one
    // primitive every mechanical mover lays a route through — refuses a
    // prisoner outright, so `tick_stock_plans`, `tick_road_parties` and the
    // finished draw in `service_sources`, which all re-lay on their own clock
    // *after* this function runs, cannot out-shout the clear below. What no
    // guard reaches is the walk that already existed: a prisoner taken
    // mid-errand still carries the movement and intent laid before the
    // seizure, and `World::step_movement` has no custody check, so a live path
    // would carry them straight out through the doorway — where the engine's
    // roam check reads a stray it did not cause, releases them, and brands
    // them with M4d's unanswerable escape notice. A person who never chose to
    // leave, wanted for the rest of the run for it.
    //
    // So a committed body has no errand and no path, re-asserted every tick
    // rather than once at commitment, because a hand-off between systems can
    // land a stale intent between any two polls and one missed clear is a
    // branding.
    let confined: Vec<ActorId> = world
        .custody
        .iter()
        .filter(|(_, record)| record.state == Confinement::Committed)
        .map(|(prisoner, _)| prisoner.clone())
        .collect();
    for prisoner in confined {
        if Some(&prisoner) == player_id.as_ref() {
            // The player's feet are never the sim's, and the host clears its own.
            continue;
        }
        if let Some(character) = world.characters.get_mut(&prisoner) {
            character.state.movement = None;
            character.state.intent = None;
        }
    }

    EscortStep {
        moved,
        committed: arrived,
    }
}

/// A departure's side of custody: tidy the law's map of somebody the round has
/// taken out of the city, and give anyone freed by it the same audible release
/// every other path gives (`law_and_order.md` M4).
///
/// The engine's `tick_custody` never sees this ending — the dead-man timer, the
/// hold ceilings and the separation poll all run on clocks, and an escort
/// walking out with a road party is not a clock — so without this the freed
/// learned nothing: no percept, no reason, a hold that ended in silence and so
/// looked exactly like one that did not. Mirrors the freed loop in
/// `tick_custody`: lingering hands come off audibly, and the prisoner is told
/// why they are free. The one voice missing is the officer's own percept —
/// they are beyond the walls, and a departure clears the inbox they would have
/// read it in.
pub fn forget_departed(world: &mut World, gone: &ActorId) {
    for (prisoner_id, record) in world.custody.forget(gone) {
        for holder in &record.holders {
            crate::actions::announce_grip(world, holder, &prisoner_id, false);
        }
        if let Some(prisoner) = world.characters.get_mut(&prisoner_id)
            && prisoner.control().is_llm()
        {
            prisoner.notify_percept(format!(
                "you are out of the law's hands: the one walking you to {} has left the city",
                record.station.name
            ));
        }
        world.touch_public_state();
    }
}

// ------------------------------------------------------------- who keeps whom

/// May this person let that one go — and, the same question asked once, is this
/// person *their keeper*?
///
/// Three ways to be, and the third is the whole of M5's *"you are confined by a
/// person, not a door"*: the officer of record, anybody with a hand on the arm,
/// or **any law-cast character standing within earshot of them**. The Stone
/// House's keeper is never the officer of record for the eight the city was
/// already holding ([`Custody::seed_inmate`] leaves that `None`), so without the
/// third arm the one person whose job this is would be the one person who could
/// not do it.
///
/// This is the shipped precondition of the `release` verb, lifted out of it so
/// that the prompt and the verb cannot disagree about who a keeper is — the
/// sheet must not offer somebody a door the action will refuse them, and it must
/// not hide one from somebody who has it.
pub fn keeps(world: &World, actor_id: &ActorId, prisoner_id: &ActorId) -> bool {
    let Some(record) = world.custody.get(prisoner_id) else {
        return false;
    };
    if record.officer.as_ref() == Some(actor_id) || record.holders.contains(actor_id) {
        return true;
    }
    let (Some(actor), Some(prisoner)) = (
        world.characters.get(actor_id),
        world.characters.get(prisoner_id),
    ) else {
        return false;
    };
    crate::notices::is_law(actor)
        && actor.position_m().distance(prisoner.position_m()) <= crate::HEARING_RADIUS_M
}

/// Is this person **keeping** anybody? What the sheet gates the keeper's
/// paragraph and the posted fee on.
///
/// Deliberately *narrower* than [`keeps`], and the difference is the point.
/// [`keeps`] answers "may this actor open that door", and its third arm is
/// [`crate::HEARING_RADIUS_M`] because a sergeant who walks up to a gate arch
/// and says *"let them out"* has always been able to. But **earshot is not a
/// post**: the Stone House stands nineteen and a half metres from the Bellstand
/// watch-bell tower, which is exactly where M2 anchored the routeless rest of
/// the bench, so a 20 m test hands the keeper's paragraph — and the posted fee,
/// and "release is yours to call on any turn" — to every debt officer and court
/// usher idling in the next yard, *through a stone wall*, on a margin of about
/// a centimetre against a nav node the bake is documented to move.
///
/// Keeping is attending: you are the officer of record, you have a hand on
/// them, or you are the law standing **at the station where they are kept**.
/// Everyone the sheet calls a keeper can still release; somebody who could
/// shout a release from the next yard simply is not told the fee, which is
/// right, because they are not the one who would be asked for it.
pub fn keeps_anyone(world: &World, actor_id: &ActorId) -> bool {
    !kept_by(world, actor_id).is_empty()
}

/// **Whom** this person is keeping, roster order — the ids `release` and `grab`
/// take, and the answer to "who is that you have there?".
///
/// Without it the Stone House's keeper would carry both verbs and no number to
/// give either: [`Custody::prisoners_of`] matches only the officer of record and
/// whoever has a hand on an arm, and the eight the city was already holding have
/// neither. A sheet that offers a verb it cannot name a target for is worse than
/// one that offers nothing.
pub fn kept_by(world: &World, actor_id: &ActorId) -> Vec<ActorId> {
    let Some(actor) = world.characters.get(actor_id) else {
        return Vec::new();
    };
    let at_post = crate::notices::is_law(actor).then(|| actor.position_m());
    world
        .custody
        .iter()
        .filter(|(_, record)| {
            record.officer.as_ref() == Some(actor_id)
                || record.holders.contains(actor_id)
                || (record.state == Confinement::Committed
                    && at_post.is_some_and(|at| {
                        f64::hypot(
                            at.x - record.station.point.x,
                            at.z - record.station.point.z,
                        ) <= STATION_ARRIVE_RADIUS_M
                    }))
        })
        .map(|(id, _)| id.clone())
        .collect()
}

// ------------------------------------------------- the city's standing inmates

/// The `circumstances` spelling (`lore.rs`'s `CONTROLLED_CIRCUMSTANCES`) that
/// says the city is already holding this person.
pub const PRISONER_CIRCUMSTANCE: &str = "prisoner";

/// How far apart the inmates stand. Close enough to be one room's conversation
/// — a broadcast `say` covers it several times over, which is the whole reason
/// the gaol works as a scene — and far enough that eleven articulated bodies do
/// not stand inside one another.
const INMATE_SPACING_M: f64 = 1.45;

/// Seed the eight the city was already holding when the run began
/// (`features/law_and_order.md` M5b), and return them in roster order.
///
/// Eight characters carry the `prisoner` circumstance and their sheets are not
/// ambiguous about it — *"you are **now held** from Bell-and-Sluice streets…
/// **Stone House rations** and food carried in by kin are your present
/// support"*. Until M5a there was nowhere to hold them, so all eight spawned
/// walking free across the whole city: a live world-consistency bug, and also
/// the gaol's entire population, already written.
///
/// **Their authored `spawn_location` is deliberately left alone.** Three reasons,
/// and they all point the same way:
///
/// 1. The room's coordinates come from the bake, not from lore — the place
///    resolves through [`stone_house`] — so eight hand-copied positions would
///    have to be re-edited every time the city is rebaked or moved (and this
///    city has been moved: it was shrunk 0.7× in 2026-07).
/// 2. `authored_spawns_cover_the_city_without_crowding` holds the authoring
///    doctrine that no 20 m neighbourhood carries more than a handful of people.
///    That rule is about *authors clumping a cast*; a gaol is the sim placing
///    people who are not free to stand anywhere else. Editing the spawns would
///    have meant weakening a real invariant to sneak a room past it.
/// 3. Their authored spawn is where their life is — and `homes.json` is baked
///    from it — so on release they still have somewhere of their own to go.
///
/// Nothing is guarded here beyond the placement: `Custody::holds` is what both
/// confinement guards test (the ladder's rung 0 in `round::decide` and the
/// `go_to` refusal in `actions.rs`), so being in the map *is* being confined,
/// and one flag with two histories is exactly the design's claim — the authored
/// eight and tonight's arrest are the same state.
pub fn seed_authored_inmates(world: &mut World) -> Vec<ActorId> {
    let Some(station) = stone_house(&world.places) else {
        return Vec::new();
    };
    let inmates: Vec<ActorId> = world
        .characters
        .iter()
        .filter(|(_, character)| {
            character.lore().is_some_and(|profile| {
                profile
                    .circumstances
                    .iter()
                    .any(|circumstance| circumstance == PRISONER_CIRCUMSTANCE)
            })
        })
        .map(|(id, _)| id.clone())
        .collect();

    let places = cell_standings(world, station.point, inmates.len());
    for (prisoner, stand) in inmates.iter().zip(places) {
        if let Some(character) = world.characters.get_mut(prisoner) {
            character.state.position_m = stand;
            // Face the middle of the room: eleven people with nothing to do but
            // talk to each other, and to you.
            character.state.facing_yaw =
                f64::atan2(stand.x - station.point.x, stand.z - station.point.z);
            // A seeded inmate never walked here, so nothing should be walking
            // them anywhere: `Decision::Stay` is a no-op and `World::step_movement`
            // has no custody check, so a live path would simply carry them out
            // through the wall.
            character.state.movement = None;
            character.state.intent = None;
        }
        world.custody.seed_inmate(prisoner.clone(), station.clone());
    }
    inmates
}

/// Where the inmates stand: concentric rings around the room's own place point,
/// keeping only walkable ground, so nobody is seeded inside a wall and a
/// released prisoner is never stranded off the graph.
fn cell_standings(world: &World, center: Vec3, count: usize) -> Vec<Vec3> {
    let mut stands = Vec::with_capacity(count);
    let walkable = |point: Vec3| {
        world
            .nav
            .as_ref()
            .is_none_or(|nav| nav.is_walkable(point.x, point.z))
    };
    // Never past half the roam, whatever the room's shape does to the walkable
    // filter: a prisoner seeded outside [`COMMITTED_ROAM_M`] would be judged to
    // have walked out on the very first poll, and eight of the lore's own
    // inmates would let themselves out of the gaol the instant the game started.
    let rings = (COMMITTED_ROAM_M * 0.5 / INMATE_SPACING_M) as usize;
    let mut ring = 0usize;
    while stands.len() < count && ring <= rings {
        let seats = if ring == 0 { 1 } else { ring * 6 };
        for seat in 0..seats {
            if stands.len() >= count {
                break;
            }
            let angle = std::f64::consts::TAU * seat as f64 / seats as f64;
            let radius = ring as f64 * INMATE_SPACING_M;
            let stand = Vec3::new(
                center.x + angle.cos() * radius,
                center.y,
                center.z + angle.sin() * radius,
            );
            if walkable(stand) {
                stands.push(stand);
            }
        }
        ring += 1;
    }
    // A room too small for everybody is still better than nobody in it: whoever
    // is left simply stands on the place point with the others.
    while stands.len() < count {
        stands.push(center);
    }
    stands
}

/// What one movement tick of the escort did. `committed` is returned rather
/// than acted on here, because arriving at a station is news — the prisoner is
/// owed a percept and the officer is owed the turn in which to say what happens
/// next — and the sim layer has no scheduler to hand it to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EscortStep {
    /// Prisoners whose position this tick changed, for the hot channel.
    pub moved: Vec<ActorId>,
    /// Prisoners who reached their station this tick.
    pub committed: Vec<ActorId>,
}

// -------------------------------------------------------------- the struggle

/// How long an unbroken pull takes to tear free of a lone, ordinary holder.
/// Escape is meant to be **easy enough to be a real choice** — what should make
/// you hesitate is the consequence, not the difficulty.
pub const STRAIN_BASE_SECONDS: f64 = 5.0;

/// The chance one NPC attempt would break a lone, ordinary holder. A little
/// better than even, for the same reason [`STRAIN_BASE_SECONDS`] is short.
const BREAK_FREE_BASE_CHANCE: f64 = 0.62;

/// How much harder than one ordinary pair of hands this grip is — **the single
/// place the struggle's modifiers live**, read by the player's strain meter and
/// the cast's one deterministic roll alike, so the two answers can never drift
/// apart. Every modifier sits on a seam that already exists.
///
/// Returns 1.0 for a lone unremarkable holder on a sober, rested body, and
/// climbs from there. `0.0` means nobody is holding anything.
pub fn grip_strength(world: &World, prisoner: &ActorId, holders: &[ActorId]) -> f64 {
    if holders.is_empty() {
        return 0.0;
    }
    // Two hands are not twice one pair: being *dragged* by two people is what
    // the word actually means, which is why the escort's right move is to shout
    // for help rather than to tighten their own grip.
    let mut strength = match holders.len() {
        1 => 1.0,
        2 => 7.0,
        _ => 20.0,
    };
    // Grip by occupation: Ede Clove should be worse to be held by than Odo
    // Trask, and her sheet already says why.
    for holder in holders {
        strength *= world
            .characters
            .get(holder)
            .and_then(|holder| holder.lore())
            .and_then(|profile| profile.occupation_id.as_deref())
            .map_or(1.0, |occupation| match occupation {
                "bailiff_and_gaoler" | "militia_and_soldier" => 1.7,
                "watchman_and_keeper" | "civic_officer" => 1.25,
                _ => 1.0,
            });
    }
    // A drunk or exhausted body pulls badly (npc_bodies M5's statuses, and the
    // drive script's `status` action, which is how this is eyeballed).
    if let Some(character) = world.characters.get(prisoner) {
        let status = |kind: StatusKind| {
            character
                .state
                .statuses
                .get(&kind)
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        };
        strength *= 1.0 + 1.5 * status(StatusKind::Drunkenness);
        strength *= 1.0 + 1.0 * status(StatusKind::Weariness);
    }
    strength
}

/// How long the player must pull without stopping to tear free of this grip.
/// Published to the host on the custody channel, because the meter is a 20 Hz
/// input affair and the sim has no clock — but the *numbers* in it are the sim's,
/// so a drunk player and a drunk NPC are hard to hold for identical reasons.
pub fn strain_seconds(world: &World, prisoner: &ActorId, holders: &[ActorId]) -> f64 {
    let strength = grip_strength(world, prisoner, holders);
    if strength <= 0.0 {
        return 0.0;
    }
    STRAIN_BASE_SECONDS * strength
}

/// The chance one NPC attempt breaks this grip — [`grip_strength`] read the
/// other way round.
pub fn break_free_chance(world: &World, prisoner: &ActorId, holders: &[ActorId]) -> f64 {
    let strength = grip_strength(world, prisoner, holders);
    if strength <= 0.0 {
        return 1.0;
    }
    (BREAK_FREE_BASE_CHANCE / strength).clamp(0.0, 1.0)
}

/// The verdict on one attempt, and the one thing about it that matters is that
/// it is **stable**: the same situation answers the same way in every run, so a
/// scripted drive reproduces and a test can assert an outcome. The same "hash,
/// never a fresh draw" idiom as `attention::opens_first` and
/// `notices::carries` — the sim contains no RNG anywhere.
pub fn struggle_roll(prisoner: &ActorId, holders: &[ActorId], attempt: u64, chance: f64) -> bool {
    if chance.is_nan() || chance <= 0.0 {
        return false;
    }
    if chance >= 1.0 {
        return true;
    }
    let mut hasher = DefaultHasher::new();
    "custody_struggle".hash(&mut hasher);
    prisoner.hash(&mut hasher);
    for holder in holders {
        holder.hash(&mut hasher);
    }
    attempt.hash(&mut hasher);
    ((hasher.finish() >> 11) as f64 / (1u64 << 53) as f64) < chance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(name: &str) -> Station {
        Station {
            place_id: PlaceId::from_raw("pl_test"),
            name: name.into(),
            point: Vec3::ZERO,
            stone_house: false,
        }
    }

    #[test]
    fn a_hold_is_refcounted_so_one_officer_letting_go_frees_nobody() {
        let (thief, ashe, mott) = (
            ActorId::from_raw("thief"),
            ActorId::from_raw("ashe"),
            ActorId::from_raw("mott"),
        );
        let mut custody = Custody::default();
        custody.seize(thief.clone(), ashe.clone(), Some(1), station("a gate"), 0.0);
        assert!(custody.holds(&thief) && !custody.is_held(&thief), "in charge, not held");

        custody.grab(&thief, ashe.clone());
        custody.grab(&thief, mott.clone());
        custody.grab(&thief, mott.clone());
        assert_eq!(custody.get(&thief).unwrap().holders.len(), 2, "and idempotent");

        custody.let_go(&thief, &ashe);
        assert!(custody.is_held(&thief), "the other hand is still on the arm");
        custody.let_go(&thief, &mott);
        assert!(!custody.is_held(&thief), "and now nobody's is");
        assert!(custody.holds(&thief), "but the custody itself stands");
    }

    /// The cap exists so a sergeant having a bad day cannot empty the
    /// Wickmarket — and the authored eight, who were already inside, never
    /// count against it.
    #[test]
    fn the_confinement_cap_counts_arrests_and_never_the_authored_inmates() {
        let mut custody = Custody::default();
        for index in 0..8 {
            custody.seed_inmate(
                ActorId::from_raw(&format!("inm{index}")),
                station("The Stone House"),
            );
        }
        assert_eq!(custody.arrest_count(), 0);
        assert!(custody.has_room());

        for index in 0..CUSTODY_MAX_ARRESTS {
            custody.seize(
                ActorId::from_raw(&format!("took{index}")),
                ActorId::from_raw("ashe"),
                Some(1),
                station("a gate"),
                0.0,
            );
        }
        assert_eq!(custody.arrest_count(), CUSTODY_MAX_ARRESTS);
        assert!(!custody.has_room(), "the law is full");

        custody.release(&ActorId::from_raw("took0"));
        assert!(custody.has_room(), "and draining is what makes room");
    }

    /// An escort who leaves the city cannot walk anybody anywhere — the same
    /// answer the dead-man timer gives, for the same reason.
    #[test]
    fn a_departed_escort_frees_the_person_they_were_walking() {
        let (thief, ashe) = (ActorId::from_raw("thief"), ActorId::from_raw("ashe"));
        let mut custody = Custody::default();
        custody.seize(thief.clone(), ashe.clone(), Some(1), station("a gate"), 0.0);
        let freed = custody.forget(&ashe);
        // The record comes back with the id, so the caller can say what ended
        // — the station by name — rather than freeing anybody in silence.
        assert_eq!(freed.len(), 1);
        assert_eq!(freed[0].0, thief);
        assert_eq!(freed[0].1.station.name, "a gate");
        assert!(!custody.holds(&thief));

        // A committed prisoner keeps their cell when the officer who brought
        // them walks out of the city: the keeper holds the threshold, not them.
        custody.seize(thief.clone(), ashe.clone(), Some(1), station("a gate"), 0.0);
        custody.commit(&thief, 1.0);
        assert!(custody.forget(&ashe).is_empty());
        assert!(custody.is_confined(&thief));
    }
}
