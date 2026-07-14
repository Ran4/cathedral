//! Characters (`sim.py:90-122`) and the percept plumbing (`sim.py:585-624`).
//!
//! The static half ([`CharacterSheet`]) is what a world seed deserializes; the
//! runtime half ([`CharacterState`]) is everything an action may mutate.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    GOAL_NONE, RECENT_HISTORY_MAX_ENTRIES,
    ids::{ActorId, ItemId},
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
        }
    }

    /// Retain one bounded, model-visible history line. Speech and other
    /// percepts share the window, including the actor's own lines.
    pub fn remember_percept(&mut self, text: impl Into<String>) {
        if !self.control().is_llm() {
            return;
        }
        self.state.recent_history.push(text.into());
        let overflow = self
            .state
            .recent_history
            .len()
            .saturating_sub(RECENT_HISTORY_MAX_ENTRIES);
        if overflow > 0 {
            self.state.recent_history.drain(..overflow);
        }
    }

    /// Deliver a new percept; it becomes short-term history once presented.
    pub fn notify_percept(&mut self, text: impl Into<String>) {
        if !self.control().is_llm() {
            return;
        }
        let text = text.into();
        self.state.inbox.push(text.clone());
        self.state.pending_history.push(text);
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
        // pending_history is unbounded — everything perceived is offered.
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
