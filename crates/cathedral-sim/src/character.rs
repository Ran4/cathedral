//! Characters (`sim.py:90-122`) and the percept plumbing (`sim.py:585-624`).
//!
//! The static half ([`CharacterSheet`]) is what a world seed deserializes; the
//! runtime half ([`CharacterState`]) is everything an action may mutate.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    GOAL_NONE, HUNGER_MAX, INBOX_MAX_ENTRIES, RECENT_HISTORY_MAX_ENTRIES, SETTLED_SPEED_MPS,
    THIRST_MAX,
    appearance::AppearanceSnapshot,
    gesture::GestureKind,
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

/// Whether a character physically participates in the city right now.
///
/// Road-party actors retain their durable state while beyond the walls, but
/// are excluded from every spatial/public seam until the party controller
/// admits them at a gate.  The serde default keeps all pre-M5 fixtures in the
/// city.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    #[default]
    InCity,
    BeyondTheWalls,
}

/// Participation in the Watch household settlement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EconomicClass {
    #[default]
    Resident,
    Visitor,
    RoadParty,
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
    /// The structured body appearance (`features/npc_bodies.md` §2), composed
    /// once at character creation ([`crate::lore`] composes it from sheet
    /// facts; seed-file characters may author it, and fixtures default it).
    #[serde(default)]
    pub appearance: AppearanceSnapshot,
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
    /// Defaults preserve old seed/fixture semantics. Production road-party
    /// membership is validated by the round and forces both fields below.
    #[serde(default)]
    pub presence: Presence,
    #[serde(default)]
    pub presence_epoch: u64,
    #[serde(default)]
    pub economic_class: EconomicClass,
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

/// A publicly-visible carriage axis (`features/npc_bodies.md` §8): a bodily
/// condition the host renders as *body language* — a drunk's sway, a weary
/// stoop — without it being a `Needs` gauge the behaviour ladder reads. Every
/// kind crosses on [`crate::ActorSnapshot::statuses`], `snake_case` so
/// `weariness` is the wire form the debug hooks parse. `Ord` because it keys a
/// [`BTreeMap`], which also fixes the snapshot's `statuses` order for free.
///
/// Nothing in the sim *sets* a status yet except the tests and the debug hooks
/// (the `cathedral-headless --status` flag and the drive-mode `status` action);
/// ale and how anyone actually gets drunk is `food_and_items` M5+ material, one
/// float write when it lands (§8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    Drunkenness,
    Weariness,
}

impl StatusKind {
    /// Every kind, in the order they cross the snapshot.
    pub const ALL: [StatusKind; 2] = [StatusKind::Drunkenness, StatusKind::Weariness];

    /// The `snake_case` wire form — the string the debug hooks parse and serde
    /// serializes to.
    pub fn as_str(self) -> &'static str {
        match self {
            StatusKind::Drunkenness => "drunkenness",
            StatusKind::Weariness => "weariness",
        }
    }

    /// Resolve the wire string a debug hook wrote to a kind, or `None` for an
    /// unknown one (the caller turns that into a usage error).
    pub fn from_wire(text: &str) -> Option<Self> {
        StatusKind::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// The dynamic drive layer — the "statuses" axis of
/// `features/movement/03_the_ladder.md` §2/§3: raw, sim-written need state the
/// behaviour ladder reads inline (`needs.thirst < THIRST_PARCHED`). Small on
/// purpose; M3 shipped thirst, food & items M2 grows hunger, with fatigue/duty
/// still following. Every gauge runs `0..=`[`THIRST_MAX`], high = satisfied.
/// Only hunger is surfaced to the prompt (a computed condition, never a raw
/// number, `features/food_and_items/03_hunger.md` §5); a world without a nav
/// graph enrols nobody, so both gauges stay full and the frozen golden fixtures
/// are byte-identical.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Needs {
    /// Falls with time; refilled at a well. The fastest gauge to decay.
    pub thirst: f64,
    /// Falls with time; refilled by eating (satiety) or the hearth. Slower and
    /// heavier than thirst — ten game hours across the gauge, not four.
    pub hunger: f64,
}

impl Default for Needs {
    /// A freshly seeded character is fully watered and fed; the round seeds a
    /// spread of starting thirst for the drawers it enrols and of hunger for
    /// everyone, so the city neither drinks nor eats in lockstep.
    fn default() -> Self {
        Self {
            thirst: THIRST_MAX,
            hunger: HUNGER_MAX,
        }
    }
}

/// One line of a bound vendor's `you_sell` price list: a kind's display name
/// and its catalog price in sparks (`05_the_llm_seam.md` §3). Written by the
/// round the moment it binds a vendor to a stall, read straight onto the sheet
/// so an LLM vendor quotes prices *off the sheet* instead of inventing them.
/// Owned strings, because the round computes the display name from the catalog
/// once at bind time; the sheet only borrows it. Empty for everyone the round
/// never made a vendor, so the section is omitted and the frozen golden
/// fixtures are byte-identical (the `you_offer` skip-when-empty pattern).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendorListing {
    /// The catalog display name of the kind sold — `"loaf"`, `"herring"`.
    pub name: String,
    /// The catalog list price of one, in sparks.
    pub price_sparks: u32,
}

/// A looping gesture the actor is currently holding (`features/npc_bodies.md`
/// §7) — today only `dance`. It surfaces on the snapshot as
/// [`crate::ActorSnapshot::active_gesture`] so a player who walks up mid-loop
/// still sees it; a one-shot gesture (a wave) needs no such state because it
/// completes before anyone new arrives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveGesture {
    pub kind: GestureKind,
    /// The absolute sim-time expiry ([`crate::DANCE_MAX_SECONDS`] after it
    /// started). Stamped by the engine on the first poll that sees the loop —
    /// the action layer has no clock, exactly like a [`TravelIntent`]'s
    /// deadline — and `None` until then.
    pub deadline: Option<f64>,
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
    /// The bound vendor's price list, rendered as the sheet's `you_sell`
    /// section: what this actor's stall charges, priced from the item catalog's
    /// stock template — not from current stock, so a sold-out baker still knows
    /// their prices (`05_the_llm_seam.md` §3). Written by
    /// [`crate::round::Round`] when it binds a vendor and cleared when it
    /// unbinds one; empty for everyone else, which omits the section and keeps
    /// the frozen golden fixtures byte-identical.
    pub you_sell: Vec<VendorListing>,
    /// The looping gesture the actor is holding, or `None`
    /// (`features/npc_bodies.md` §7). Set by the `gesture` verb for `dance`,
    /// cleared by the actor's next non-`wait` action or by the engine after
    /// [`crate::DANCE_MAX_SECONDS`]. Never rendered on the prompt — it is body
    /// language for the host, not something the mind reads back off its sheet.
    pub active_gesture: Option<ActiveGesture>,
    /// Set while a road party owns the member's gateward movement.
    pub leaving_city: bool,
    /// Publicly-visible carriage axes (`features/npc_bodies.md` §8): drunkenness,
    /// weariness. Each value runs `0..=1`; the host reads them off the snapshot
    /// and dresses the walk (sway, stoop) without touching the position the sim
    /// computed. Never rendered on the prompt — a status is a fact about the
    /// body, not a fact the mind reads back. Empty for everyone the debug hooks
    /// (or, later, an ale) never touched, which keeps the snapshot byte-identical.
    pub statuses: BTreeMap<StatusKind, f64>,
    /// Physical-world participation and its monotonic incarnation stamp.
    pub presence: Presence,
    pub presence_epoch: u64,
    /// Watch-settlement participation. This is state, not a prompt fact.
    pub economic_class: EconomicClass,
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
            you_sell: Vec::new(),
            active_gesture: None,
            leaving_city: false,
            statuses: BTreeMap::new(),
            presence: sheet.presence,
            presence_epoch: sheet.presence_epoch,
            economic_class: if sheet.control == Control::Player {
                EconomicClass::Visitor
            } else {
                sheet.economic_class
            },
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

    pub fn appearance(&self) -> &AppearanceSnapshot {
        &self.sheet.appearance
    }

    /// The looping gesture the actor is currently holding, for the public
    /// snapshot (`features/npc_bodies.md` §7).
    pub fn active_gesture(&self) -> Option<GestureKind> {
        self.state.active_gesture.map(|active| active.kind)
    }

    /// The publicly-visible carriage statuses for the snapshot
    /// (`features/npc_bodies.md` §8), each bounded to a finite `0..=1` and
    /// ordered by kind (the [`BTreeMap`] already is). Non-finite or
    /// out-of-range values are clamped here so the boundary only ever sees
    /// clean data — the host and its validator clamp again as a defensive gate.
    pub fn statuses(&self) -> Vec<(StatusKind, f32)> {
        self.state
            .statuses
            .iter()
            .map(|(&kind, &value)| (kind, clamp_status(value)))
            .collect()
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

/// A carriage status value, forced to a finite `0..=1` and narrowed to the f32
/// the snapshot carries. Non-finite (a debug hook, or a future ale, writing a
/// NaN) reads as 0 — no carriage rather than an undefined pose.
fn clamp_status(value: f64) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0) as f32
    } else {
        0.0
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
            appearance: AppearanceSnapshot::default(),
            voice_key: None,
            position_m: Vec3::ZERO,
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: goal_none(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: Presence::InCity,
            presence_epoch: 0,
            economic_class: EconomicClass::Resident,
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

    /// §8: the carriage kinds serialize to their `snake_case` wire word — the
    /// exact string the debug hooks parse and the snapshot carries — and every
    /// kind round-trips through it.
    #[test]
    fn status_kind_serializes_snake_case_and_round_trips() {
        assert_eq!(
            serde_json::to_string(&StatusKind::Drunkenness).unwrap(),
            "\"drunkenness\""
        );
        assert_eq!(
            serde_json::to_string(&StatusKind::Weariness).unwrap(),
            "\"weariness\""
        );
        assert_eq!(StatusKind::ALL.len(), 2);
        for kind in StatusKind::ALL {
            assert_eq!(StatusKind::from_wire(kind.as_str()), Some(kind));
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<StatusKind>(&json).unwrap(), kind);
        }
        assert_eq!(StatusKind::from_wire("sobriety"), None);
    }

    /// `statuses()` exposes the map bounded to a finite `0..=1` and ordered by
    /// kind (the `BTreeMap` already is), so the boundary only sees clean data.
    #[test]
    fn statuses_are_exposed_bounded_and_ordered() {
        let mut character = Character::from_sheet(sheet(Control::Llm));
        assert!(character.statuses().is_empty());

        // Insert out of order and out of range; `statuses()` clamps and sorts.
        character.state.statuses.insert(StatusKind::Weariness, 1.4);
        character.state.statuses.insert(StatusKind::Drunkenness, -0.2);
        assert_eq!(
            character.statuses(),
            vec![
                (StatusKind::Drunkenness, 0.0),
                (StatusKind::Weariness, 1.0)
            ]
        );

        // Non-finite reads as no carriage; a clean value passes through exactly.
        character.state.statuses.insert(StatusKind::Weariness, f64::NAN);
        character.state.statuses.insert(StatusKind::Drunkenness, 0.5);
        assert_eq!(
            character.statuses(),
            vec![
                (StatusKind::Drunkenness, 0.5),
                (StatusKind::Weariness, 0.0)
            ]
        );
    }
}
