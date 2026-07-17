//! Characters (`sim.py:90-122`) and the percept plumbing (`sim.py:585-624`).
//!
//! The static half ([`CharacterSheet`]) is what a world seed deserializes; the
//! runtime half ([`CharacterState`]) is everything an action may mutate.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    GOAL_NONE, INBOX_MAX_ENTRIES, RECENT_HISTORY_MAX_ENTRIES, SETTLED_SPEED_MPS, THIRST_MAX,
    ids::{ActorId, ItemId, PlaceId},
    lore::{LoreProfile, Significance},
    math::Vec3,
};

/// Who decides this character's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Llm,
    Player,
}

impl Control {
    /// Only LLM-controlled characters accumulate prose percepts: the player
    /// gets structured events instead, and is never scheduled.
    pub fn is_llm(self) -> bool {
        matches!(self, Self::Llm)
    }
}

fn goal_none() -> String {
    GOAL_NONE.to_string()
}

/// Static identity plus seed values — the serde "character sheet".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSheet {
    pub id: ActorId,
    pub name: String,
    pub control: Control,
    pub back_story: String,
    pub location_description: String,
    pub appearance_key: String,
    /// `None` for the player by convention; never in the public snapshot.
    pub voice_key: Option<String>,
    /// Spawn position.
    #[serde(with = "crate::math::vec3_serde")]
    pub position_m: Vec3,
    /// Compass bearing in radians, matching Bevy: yaw 0 faces -Z, so the
    /// character faces `(-sin(yaw), -cos(yaw))` in the XZ plane.
    #[serde(default)]
    pub facing_yaw: f64,
    #[serde(default)]
    pub holds: Vec<ItemId>,
    /// The `"None"` string sentinel, not an Option — `set_goal` compares
    /// against it and the prompt renders it directly (D15).
    #[serde(default = "goal_none")]
    pub goal: String,
    #[serde(default)]
    pub memories: Vec<String>,
    /// Seeded at world creation. Human observers add a speaker when they hear
    /// that speaker say their own full name.
    #[serde(default)]
    pub knows: BTreeSet<ActorId>,
    /// Complete authored metadata for lore-backed NPCs. The player and compact
    /// test fixtures intentionally have no lore profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lore: Option<LoreProfile>,
}

/// The M2 ping-pong: an actor walks back and forth between two named nav places.
/// M4 replaces this with the character's real daily round, so it stays a plain
/// pair of place names plus which end we are currently heading for.
#[derive(Debug, Clone, PartialEq)]
pub struct Patrol {
    pub a: String,
    pub b: String,
    /// True while the next arrival is `b`; flipped each time the path empties.
    pub heading_to_b: bool,
}

/// A character's live walk: the remaining polyline, the current gait, and an
/// optional [`Patrol`] that refills the path once it empties. `None` on a
/// character means it never walks (the player, a statue);
/// [`CharacterState::from_sheet`] starts it `None` and only the mover pipeline
/// ever sets it.
#[derive(Debug, Clone, PartialEq)]
pub struct Movement {
    /// Remaining waypoints; the next target is `path[0]`. Empty means arrived.
    pub path: Vec<Vec3>,
    /// Current speed in m/s — 0 when arrived or idle.
    pub speed: f64,
    /// Continuous walk cadence, advanced by `speed * dt * k` each slice; M7
    /// renders the bob and swing from it. Never reset, so the gait is seamless
    /// across legs.
    pub gait_phase: f64,
    /// `Some` — the M2 ping-pong: [`crate::World::step_movement`] flips it and
    /// re-routes to the far end when the path empties. `None` — the walk simply
    /// stops on arrival (speed → 0) and a higher layer decides what happens next;
    /// this is what the M3 water round uses, since the behaviour ladder owns the
    /// arrival, not the mover (`features/movement/03_the_ladder.md` §4).
    pub patrol: Option<Patrol>,
    /// Seconds spent standing at the mouth of an occupied one-person choke (M7:
    /// the Needle's claim). 0 while moving freely; when it exceeds the reroute
    /// budget the mover tries the long way round instead
    /// (`features/movement/02_navigation.md` §5: "you wait, or you take Cinder
    /// Row").
    pub choke_wait: f64,
}

/// The dynamic drive layer — the "statuses" axis of
/// `features/movement/03_the_ladder.md` §2/§3: raw, sim-written need state the
/// behaviour ladder reads inline (`needs.thirst < THIRST_PARCHED`). Small on
/// purpose; M3 ships thirst only, with hunger/fatigue/duty following in M4.
/// Every gauge runs `0..=`[`THIRST_MAX`], high = satisfied. Never rendered into
/// the prompt in M3 (that is M5's sheet change), so a world without a nav graph
/// leaves the frozen golden fixtures byte-identical.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Needs {
    /// Falls with time; refilled at a well. The fastest gauge to decay.
    pub thirst: f64,
}

impl Default for Needs {
    /// A freshly seeded character is fully watered; the water round seeds a
    /// spread of starting values for the drawers it enrolls.
    fn default() -> Self {
        Self { thirst: THIRST_MAX }
    }
}

/// Where a [`TravelIntent`] is headed.
#[derive(Debug, Clone, PartialEq)]
pub enum IntentTarget {
    /// A handle from `places_you_know`. The name and point are captured at
    /// intent time so the percepts that end the errand need no registry.
    Place {
        place_id: PlaceId,
        name: String,
        point: Vec3,
    },
    /// Someone the actor could *see* when the intent was set — the sight gate
    /// that keeps a hoarded id from becoming a tracking device
    /// (`features/movement/05_the_llm_seam.md` §2). While they stay visible the
    /// follow tracks them; losing sight degrades the intent to `last_seen`.
    Person {
        actor_id: ActorId,
        last_seen: Vec3,
        visible: bool,
    },
}

/// An LLM-issued `go_to`: an intent, not a move. The verb returns immediately
/// and the behaviour ladder carries the body there over the next seconds or
/// minutes — a suggestion layered on an already-autonomous agent, never its
/// brain (05_the_llm_seam.md §2). It expires on a route-derived budget, the
/// pressing needs preempt it, and both endings are percepts.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelIntent {
    pub target: IntentTarget,
    /// Real-seconds lifetime, priced from the route at intent time
    /// ([`crate::GO_TO_BUDGET_FACTOR`] × expected travel time, floored) —
    /// without it one confused reply strands a character permanently.
    pub budget_seconds: f64,
    /// The absolute expiry, stamped by the round on the first tick that sees
    /// the intent — the action layer has no clock to stamp it with.
    pub deadline: Option<f64>,
}

/// Everything an action may change.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterState {
    pub position_m: Vec3,
    pub facing_yaw: f64,
    /// Ordered: accept appends to the end, removal preserves the rest.
    pub holds: Vec<ItemId>,
    pub goal: String,
    /// Insertion-ordered, deduped by exact string on `remember`.
    pub memories: Vec<String>,
    /// Live perspective knowledge; normally static for NPCs, expanded for a
    /// human observer by a heard self-introduction.
    pub knows: BTreeSet<ActorId>,
    /// Unread prose percepts; only ever appended for `control == llm`.
    pub inbox: Vec<String>,
    /// Bounded percept window (speech + sounds only).
    pub recent_history: Vec<String>,
    /// Percepts delivered but not yet presented in a prompt. Disjoint from
    /// `recent_history` at all times.
    pub pending_history: Vec<String>,
    /// The live walk, or `None` for a character that never moves. Not seeded
    /// from the sheet — the mover pipeline sets it — so a world with no nav graph
    /// has nobody walking and the frozen fixtures are unaffected.
    pub movement: Option<Movement>,
    /// The dynamic drive gauges the behaviour ladder reads. Seeded to
    /// [`Needs::default`] (fully satisfied); the water round overrides the
    /// starting thirst of the drawers it enrolls. Never rendered in M3, so the
    /// golden fixtures stay byte-identical.
    pub needs: Needs,
    /// The wayfinding whitelist — which [`PlaceId`] handles this character
    /// holds, i.e. the sheet's `places_you_know`. Seeded by the round (home,
    /// workplace, the legs of the day, the own ward's places, the coarse
    /// destinations everyone has), grown by `tell_way`: the verb writes here
    /// and the sheet *is* the model's memory (05_the_llm_seam.md §3).
    pub places_known: BTreeSet<PlaceId>,
    /// The live `go_to` errand, or `None`. A second `go_to` replaces it
    /// silently; `stop {}` clears it; arrival, expiry and a pressing need end
    /// it with a percept.
    pub intent: Option<TravelIntent>,
    /// The actor's own knowledge of their daily round, one prose line per leg
    /// ("at Dayspring: work at The Wickmarket"), written once by
    /// [`crate::round::Round::seed`] and rendered as the sheet's `your_round`
    /// section. Empty for anyone the round never enrolled — the section is
    /// then omitted, keeping the frozen golden fixtures byte-identical.
    pub daily_round: Vec<String>,
}

impl CharacterState {
    pub fn from_sheet(sheet: &CharacterSheet) -> Self {
        Self {
            position_m: sheet.position_m,
            facing_yaw: sheet.facing_yaw,
            holds: sheet.holds.clone(),
            goal: sheet.goal.clone(),
            memories: sheet.memories.clone(),
            knows: sheet.knows.clone(),
            inbox: Vec::new(),
            recent_history: Vec::new(),
            pending_history: Vec::new(),
            movement: None,
            needs: Needs::default(),
            places_known: BTreeSet::new(),
            intent: None,
            daily_round: Vec::new(),
        }
    }
}

/// A character in the world: the sheet it was seeded from plus live state.
#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub sheet: CharacterSheet,
    pub state: CharacterState,
}

impl Character {
    pub fn from_sheet(sheet: CharacterSheet) -> Self {
        let state = CharacterState::from_sheet(&sheet);
        Self { sheet, state }
    }

    pub fn id(&self) -> &ActorId {
        &self.sheet.id
    }

    pub fn name(&self) -> &str {
        &self.sheet.name
    }

    pub fn control(&self) -> Control {
        self.sheet.control
    }

    pub fn appearance_key(&self) -> &str {
        &self.sheet.appearance_key
    }

    pub fn voice_key(&self) -> Option<&str> {
        self.sheet.voice_key.as_deref()
    }

    pub fn back_story(&self) -> &str {
        &self.sheet.back_story
    }

    pub fn location_description(&self) -> &str {
        &self.sheet.location_description
    }

    pub fn position_m(&self) -> Vec3 {
        self.state.position_m
    }

    pub fn facing_yaw(&self) -> f64 {
        self.state.facing_yaw
    }

    /// The character's current walking speed in m/s — 0 for anyone not walking.
    pub fn speed(&self) -> f64 {
        self.state.movement.as_ref().map_or(0.0, |movement| movement.speed)
    }

    /// Whether the character is settled enough to count as "present" for the
    /// novelty gate. A mover crossing the square is not news at every step; a man
    /// who has stopped — and the player, who never carries a [`Movement`] — is
    /// (`features/movement/05_the_llm_seam.md` §5.1).
    pub fn is_settled(&self) -> bool {
        self.speed() < SETTLED_SPEED_MPS
    }

    /// The character's dynamic need gauges (thirst, …).
    pub fn needs(&self) -> Needs {
        self.state.needs
    }

    /// Whether the character currently has waypoints left to walk. A mover whose
    /// path has emptied has *arrived*, which is how the water round's arrival
    /// transitions are detected.
    pub fn is_walking(&self) -> bool {
        self.state
            .movement
            .as_ref()
            .is_some_and(|movement| !movement.path.is_empty())
    }

    pub fn holds(&self) -> &[ItemId] {
        &self.state.holds
    }

    pub fn goal(&self) -> &str {
        &self.state.goal
    }

    pub fn memories(&self) -> &[String] {
        &self.state.memories
    }

    pub fn knows(&self) -> &BTreeSet<ActorId> {
        &self.state.knows
    }

    pub fn lore(&self) -> Option<&LoreProfile> {
        self.sheet.lore.as_ref()
    }

    /// Non-lore fixtures keep the legacy full-cadence behavior. Production
    /// NPCs always carry an explicit significance in their lore profile.
    pub fn significance(&self) -> Significance {
        self.lore()
            .map_or(Significance::Major, |profile| profile.significance)
    }

    pub fn inbox(&self) -> &[String] {
        &self.state.inbox
    }

    pub fn recent_history(&self) -> &[String] {
        &self.state.recent_history
    }

    pub fn pending_history(&self) -> &[String] {
        &self.state.pending_history
    }

    /// Queue private prose — only for actors whose scheduler consumes it.
    pub fn notify(&mut self, text: impl Into<String>) {
        if self.control().is_llm() {
            self.state.inbox.push(text.into());
            cap_front(&mut self.state.inbox, INBOX_MAX_ENTRIES);
        }
    }

    /// Retain one bounded, model-visible history line. Speech and other
    /// percepts share the window, including the actor's own lines.
    ///
    /// A line that repeats the newest entry coalesces into it with a running
    /// count — `boom (3 times now)` — so a percept barrage (a player spamming
    /// one sound, the town bell) cannot flush real dialogue out of the window,
    /// and the count itself carries the escalation. Only *consecutive*
    /// duplicates coalesce: `boom, "hello", boom` stays three entries, because
    /// the interleaving is itself information.
    pub fn remember_percept(&mut self, text: impl Into<String>) {
        if !self.control().is_llm() {
            return;
        }
        let text = text.into();
        if let Some(last) = self.state.recent_history.last_mut() {
            let (base, count) = split_repeat_count(last);
            if base == text {
                *last = format!("{text} ({} times now)", count.saturating_add(1));
                return;
            }
        }
        self.state.recent_history.push(text);
        cap_front(&mut self.state.recent_history, RECENT_HISTORY_MAX_ENTRIES);
    }

    /// Deliver a new percept; it becomes short-term history once presented.
    pub fn notify_percept(&mut self, text: impl Into<String>) {
        if !self.control().is_llm() {
            return;
        }
        let text = text.into();
        self.state.inbox.push(text.clone());
        self.state.pending_history.push(text);
        // An actor the stage gate never prompts drains neither, so both are
        // bounded here — the oldest, stalest percepts fall off first.
        cap_front(&mut self.state.inbox, INBOX_MAX_ENTRIES);
        cap_front(&mut self.state.pending_history, INBOX_MAX_ENTRIES);
    }

    /// Detach the percepts a prompt is about to present as
    /// `since_your_last_turn`. A turn that never completes must push them back
    /// onto the FRONT of `pending_history` so a retried prompt presents them as
    /// new again (`scheduler.py:228-229`).
    pub fn take_pending_history(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.pending_history)
    }

    /// Graduate the percepts a completed turn presented into `recent_history`.
    pub fn absorb_presented_history(&mut self, presented: &[String]) {
        for text in presented {
            self.remember_percept(text.clone());
        }
    }

    /// Re-apply the [`INBOX_MAX_ENTRIES`] cap after a path that mutated the
    /// percept buffers directly rather than through [`notify_percept`].
    ///
    /// The scheduler's failure / busy / prompt-failed paths restore drained
    /// percepts onto the front and append a `system:` line without going through
    /// the capping methods, so a run of provider failures would otherwise grow
    /// `inbox` and `pending_history` one line past the bound each time, without
    /// limit (`features/movement/05_the_llm_seam.md` §5.3;
    /// `features/movement/code_review.md` finding 2). This keeps the same
    /// invariant those methods do — the oldest, stalest lines fall off the front.
    pub fn rebound_percepts(&mut self) {
        cap_front(&mut self.state.inbox, INBOX_MAX_ENTRIES);
        cap_front(&mut self.state.pending_history, INBOX_MAX_ENTRIES);
    }
}

/// A history entry's base line and how many consecutive times it has occurred:
/// `"boom (3 times now)"` → `("boom", 3)`, anything else → `(entry, 1)`.
fn split_repeat_count(entry: &str) -> (&str, u32) {
    let Some(rest) = entry.strip_suffix(" times now)") else {
        return (entry, 1);
    };
    match rest.rsplit_once(" (") {
        Some((base, digits)) if digits.bytes().all(|b| b.is_ascii_digit()) => {
            digits.parse().map_or((entry, 1), |count| (base, count))
        }
        _ => (entry, 1),
    }
}

/// Drop the oldest entries so `buffer` holds at most `max`, preserving order.
/// The shared shape of every bounded percept window in this file.
fn cap_front(buffer: &mut Vec<String>, max: usize) {
    let overflow = buffer.len().saturating_sub(max);
    if overflow > 0 {
        buffer.drain(..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(control: Control) -> CharacterSheet {
        CharacterSheet {
            id: ActorId::from_raw("a"),
            name: "A".into(),
            control,
            back_story: String::new(),
            location_description: String::new(),
            appearance_key: String::new(),
            voice_key: None,
            position_m: Vec3::ZERO,
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: goal_none(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
        }
    }

    /// Written against the bound itself, not a copy of its value: this test is
    /// what says raising `RECENT_HISTORY_MAX_ENTRIES` keeps the *newest* lines,
    /// and it stops saying that the moment it hardcodes the old number.
    #[test]
    fn recent_history_is_bounded_and_drops_the_oldest() {
        const OVERFLOW: usize = 3;
        let offered = RECENT_HISTORY_MAX_ENTRIES + OVERFLOW;

        let mut character = Character::from_sheet(sheet(Control::Llm));
        for index in 0..offered {
            character.notify_percept(format!("line {index}"));
        }
        // Well under INBOX_MAX_ENTRIES, so pending_history still holds them all.
        let presented = character.take_pending_history();
        assert_eq!(presented.len(), offered);
        assert!(character.pending_history().is_empty());

        character.absorb_presented_history(&presented);
        let history = character.recent_history();
        assert_eq!(history.len(), RECENT_HISTORY_MAX_ENTRIES);
        // The three oldest fell off the front; the newest line survived.
        assert_eq!(history[0], format!("line {OVERFLOW}"));
        assert_eq!(
            history[RECENT_HISTORY_MAX_ENTRIES - 1],
            format!("line {}", offered - 1)
        );
    }

    /// A character the scheduler never prompts still receives sounds and speech,
    /// so its inbox and pending_history are bounded — the oldest, stalest lines
    /// fall off rather than growing for the whole session
    /// (`features/movement/05_the_llm_seam.md` §5.3).
    #[test]
    fn the_never_prompted_inbox_is_bounded_and_keeps_the_newest() {
        const OVERFLOW: usize = 5;
        let offered = INBOX_MAX_ENTRIES + OVERFLOW;

        let mut character = Character::from_sheet(sheet(Control::Llm));
        for index in 0..offered {
            character.notify_percept(format!("line {index}"));
        }
        // Both buffers cap at the bound, and it is the newest lines they keep.
        for buffer in [character.inbox(), character.pending_history()] {
            assert_eq!(buffer.len(), INBOX_MAX_ENTRIES);
            assert_eq!(buffer[0], format!("line {OVERFLOW}"));
            assert_eq!(buffer[INBOX_MAX_ENTRIES - 1], format!("line {}", offered - 1));
        }

        // `notify` (private prose, no pending_history) caps the inbox too.
        let mut chatterer = Character::from_sheet(sheet(Control::Llm));
        for index in 0..offered {
            chatterer.notify(format!("note {index}"));
        }
        assert_eq!(chatterer.inbox().len(), INBOX_MAX_ENTRIES);
        assert!(chatterer.pending_history().is_empty());
    }

    /// The barrage case from
    /// `features/small_thing_deduplicate_repeat_recent_history.md`: consecutive
    /// identical percepts collapse into one counted entry, while an interleaved
    /// line keeps repeats apart — the ordering is itself information.
    #[test]
    fn consecutive_duplicate_percepts_coalesce_with_a_count() {
        let mut character = Character::from_sheet(sheet(Control::Llm));
        character.remember_percept("[You heard a big fart!]");
        character.remember_percept("[You heard a big fart!]");
        character.remember_percept("[You heard a big fart!]");
        assert_eq!(
            character.recent_history(),
            ["[You heard a big fart!] (3 times now)"]
        );

        // A different line breaks the run; the same percept afterwards is a
        // fresh entry, not an increment of the earlier one.
        character.remember_percept("Anna says: \"hello\"");
        character.remember_percept("[You heard a big fart!]");
        assert_eq!(
            character.recent_history(),
            [
                "[You heard a big fart!] (3 times now)",
                "Anna says: \"hello\"",
                "[You heard a big fart!]",
            ]
        );
    }

    /// Duplicates coalesce across the graduation path too: percepts presented
    /// as `since_your_last_turn` stay per-event there (the delta is drained
    /// each turn and its rate limits bound it), but collapse when they land in
    /// `recent_history`.
    #[test]
    fn graduated_percepts_coalesce_but_the_inbox_delta_does_not() {
        let mut character = Character::from_sheet(sheet(Control::Llm));
        character.notify_percept("[The town bell tolls.]");
        character.notify_percept("[The town bell tolls.]");

        let presented = character.take_pending_history();
        assert_eq!(
            presented,
            ["[The town bell tolls.]", "[The town bell tolls.]"]
        );

        character.absorb_presented_history(&presented);
        assert_eq!(
            character.recent_history(),
            ["[The town bell tolls.] (2 times now)"]
        );

        // The next round continues the same run.
        character.remember_percept("[The town bell tolls.]");
        assert_eq!(
            character.recent_history(),
            ["[The town bell tolls.] (3 times now)"]
        );
    }

    #[test]
    fn split_repeat_count_only_matches_the_counted_suffix() {
        assert_eq!(split_repeat_count("boom"), ("boom", 1));
        assert_eq!(split_repeat_count("boom (3 times now)"), ("boom", 3));
        // Near-misses stay opaque strings rather than half-parsing.
        assert_eq!(
            split_repeat_count("boom (many times now)"),
            ("boom (many times now)", 1)
        );
        assert_eq!(split_repeat_count("(3 times now)"), ("(3 times now)", 1));
    }

    #[test]
    fn players_accumulate_no_prose() {
        let mut character = Character::from_sheet(sheet(Control::Player));
        character.notify("inbox");
        character.notify_percept("percept");
        character.remember_percept("history");
        assert!(character.inbox().is_empty());
        assert!(character.pending_history().is_empty());
        assert!(character.recent_history().is_empty());
    }
}
