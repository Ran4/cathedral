//! Who is allowed to think, and when.
//!
//! The turn stream has always been a clock: the round robin handed out slots
//! whether or not anybody was there to notice. This module is the answer to the
//! rule a player would state — an NPC thinks because you are near them, because
//! you spoke to them, or because something happened to them, never merely
//! because their name came up in a rotation
//! (`features/gate_idle_cognition_on_proximity.md`).
//!
//! It is pure and cheap: [`on_stage`] is a distance query the engine recomputes
//! once per poll, and [`IdleGate`] is all the scheduler ever learns about it.
//! Nothing here gates the lanes that fire on a *real* event — an addressed
//! `say`, a sound nudge, the player's own speech. Those reach an ambient
//! fishmonger in an empty field exactly as they always did.

use std::collections::BTreeSet;

use crate::{character::Control, ids::ActorId, world::World};

/// Deliberately larger than `HEARING_RADIUS_M` (20 m).
///
/// The tell this feature risks is statues that come alive when looked at. A
/// generous radius is the cheap mitigation: by the time you are in earshot, the
/// NPC has been taking turns for the better part of a minute.
pub const DEFAULT_STAGE_RADIUS_M: f64 = 32.0;

/// How many neighbours may idle at once. Not a cost cap on concurrency — the
/// scheduler still allows exactly one request in flight — but on how thinly the
/// rotation is spread: six people around you each get a turn every ~19 s, where
/// 240 people would each get one every 13 minutes.
pub const DEFAULT_STAGE_MAX_ACTORS: usize = 6;

/// `characters_within` demands a finite radius, and a fat-fingered `config.ron`
/// must not panic the engine. The city is about 1.2 × 1.0 km, so this bound is
/// "the whole world" in practice — use `IdleCognitionMode::All` if that is what
/// you actually meant.
pub const MAX_STAGE_RADIUS_M: f64 = 2_000.0;

/// How long the player's conversation partner keeps their reserved seat after
/// the last line either of them addressed to the other.
///
/// Long enough to cover walking backwards out of the radius mid-sentence; short
/// enough that a conversation you simply walked away from stops costing
/// anything. Without a lapse the partner would idle forever, and "alone in a
/// field" would cost one prompt per rotation for the rest of the run.
pub const STAGE_PARTNER_MEMORY_SECONDS: f64 = 30.0;

/// Whether the idle lane is gated on the player's neighborhood at all.
///
/// `All` reproduces the pre-gate behavior exactly, which is what makes this
/// feature a one-line A/B in `config.ron` and what keeps the scheduler tests and
/// the headless runner exercising the full cast without faking proximity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleCognitionMode {
    #[default]
    All,
    Stage,
}

impl IdleCognitionMode {
    /// `config.ron` carries the mode as a string, like `tts_backend` and
    /// `stt_backend`. Anything but an explicit `"all"` gates.
    pub fn from_config(value: &str) -> Self {
        if value.trim().eq_ignore_ascii_case("all") {
            Self::All
        } else {
            Self::Stage
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Stage => "stage",
        }
    }
}

/// The player's neighborhood, in metres and in bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageConfig {
    pub radius_m: f64,
    pub max_actors: usize,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: DEFAULT_STAGE_MAX_ACTORS,
        }
    }
}

/// Which actors the scheduler's *idle* lanes may select this poll.
///
/// The protected player-reaction lane is never gated by this — the player spoke,
/// so someone answers — and neither variant can start a turn that is not in
/// `order` to begin with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleGate<'a> {
    /// Every actor in the rotation may idle. Today's behavior.
    All,
    /// Only these may: the set [`on_stage`] computed for this poll.
    Stage(&'a BTreeSet<ActorId>),
    /// Nobody may — not even the ordinary priority slot. The player is
    /// mid-utterance, and the one in-flight slot is about to be his.
    ///
    /// The scheduler cannot preempt a provider call once it is out, so the only
    /// way to keep the player's words from queueing behind two seconds of some
    /// irrelevant NPC's thinking is to not start that turn at all.
    Suppressed,
}

impl IdleGate<'_> {
    pub fn allows(&self, actor_id: &ActorId) -> bool {
        match self {
            Self::All => true,
            Self::Stage(stage) => stage.contains(actor_id),
            Self::Suppressed => false,
        }
    }

    /// Whether even an event-driven handoff should wait.
    pub fn is_suppressed(&self) -> bool {
        matches!(self, Self::Suppressed)
    }
}

/// The actors eligible for an idle turn: the player's neighborhood, nearest
/// first, plus whoever the player is currently in an exchange with.
///
/// `partner` keeps a reserved seat — it is exactly the case where distance has
/// stopped being the right question. The engine supplies it from the last
/// targeted line the player and an NPC exchanged, and lets it lapse after
/// [`STAGE_PARTNER_MEMORY_SECONDS`].
///
/// `World::characters_within` already orders by distance then id, so the cap is
/// free and the result is deterministic.
pub fn on_stage(
    world: &World,
    player_id: &ActorId,
    partner: Option<&ActorId>,
    config: &StageConfig,
) -> BTreeSet<ActorId> {
    let mut stage = BTreeSet::new();
    if let Some(partner) = partner.filter(|actor_id| is_llm(world, actor_id)) {
        stage.insert(partner.clone());
    }
    let Some(player) = world.characters.get(player_id) else {
        return stage;
    };
    // NOT `clamp`, which clippy will offer and which would be a bug: `clamp`
    // returns NaN for a NaN input, and `characters_within` asserts the radius is
    // finite. `f64::max` returns the other operand for NaN, so this chain
    // sanitizes a garbage `config.ron` radius to zero instead of panicking the
    // engine (`a_garbage_radius_empties_the_stage_instead_of_panicking`).
    #[allow(clippy::manual_clamp)]
    let radius_m = config.radius_m.max(0.0).min(MAX_STAGE_RADIUS_M);
    for actor_id in world.characters_within(player.position_m(), radius_m, Some(player_id)) {
        if stage.len() >= config.max_actors {
            break;
        }
        // The player is already excluded; this drops any other non-LLM body, and
        // a nearby partner simply re-inserts as a no-op.
        if is_llm(world, &actor_id) {
            stage.insert(actor_id);
        }
    }
    stage
}

fn is_llm(world: &World, actor_id: &ActorId) -> bool {
    world
        .characters
        .get(actor_id)
        .is_some_and(|character| character.control() == Control::Llm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CharacterSheet, Vec3,
        character::{Character, Control},
    };

    fn character(id: &str, x: f64, control: Control) -> Character {
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control,
            back_story: "test".into(),
            location_description: "the square".into(),
            appearance_key: id.into(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: crate::GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
        })
    }

    /// The player at the origin, with company at 5, 15, 25 and 100 metres.
    fn world() -> World {
        let mut world = World::new();
        world.add_character(character("player", 0.0, Control::Player));
        world.add_character(character("near", 5.0, Control::Llm));
        world.add_character(character("mid", 15.0, Control::Llm));
        world.add_character(character("far", 25.0, Control::Llm));
        world.add_character(character("away", 100.0, Control::Llm));
        world
    }

    fn ids(stage: &BTreeSet<ActorId>) -> Vec<&str> {
        stage.iter().map(ActorId::as_str).collect()
    }

    fn player() -> ActorId {
        ActorId::from_raw("player")
    }

    #[test]
    fn the_stage_is_the_neighborhood_and_never_the_player() {
        let stage = on_stage(&world(), &player(), None, &StageConfig::default());
        // 32 m by default: `far` is in, `away` is not, and the player himself is
        // never scheduled.
        assert_eq!(ids(&stage), ["far", "mid", "near"]);
    }

    #[test]
    fn an_empty_field_costs_nothing() {
        let mut world = world();
        // Walk the player 500 m out. This is the headline number: no idle lane,
        // no prompt, no bill.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(500.0, 0.0, 0.0);
        assert!(on_stage(&world, &player(), None, &StageConfig::default()).is_empty());
    }

    #[test]
    fn the_cap_keeps_the_nearest_and_drops_the_rest() {
        let config = StageConfig {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: 2,
        };
        // Nearest-first truncation, even though the set itself sorts by id: the
        // 25 m `far` is the one dropped, not the alphabetically-last `near`.
        assert_eq!(
            ids(&on_stage(&world(), &player(), None, &config)),
            ["mid", "near"]
        );
    }

    #[test]
    fn the_conversation_partner_keeps_a_seat_outside_the_radius() {
        let away = ActorId::from_raw("away");
        let config = StageConfig {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: 2,
        };
        let stage = on_stage(&world(), &player(), Some(&away), &config);
        // The partner is seated first and the cap spends what is left on the
        // nearest neighbour — you keep talking to the person you walked away
        // from, and the person beside you still lives.
        assert_eq!(ids(&stage), ["away", "near"]);
    }

    #[test]
    fn a_non_llm_partner_is_not_smuggled_onto_the_stage() {
        let mut world = world();
        world.add_character(character("statue", 200.0, Control::Player));
        let statue = ActorId::from_raw("statue");
        let stage = on_stage(&world, &player(), Some(&statue), &StageConfig::default());
        assert_eq!(ids(&stage), ["far", "mid", "near"]);
    }

    #[test]
    fn a_garbage_radius_empties_the_stage_instead_of_panicking() {
        for radius_m in [f64::NAN, -1.0] {
            let config = StageConfig {
                radius_m,
                max_actors: DEFAULT_STAGE_MAX_ACTORS,
            };
            assert!(on_stage(&world(), &player(), None, &config).is_empty());
        }
        // Infinity clamps to the whole world rather than tripping the assertion.
        let config = StageConfig {
            radius_m: f64::INFINITY,
            max_actors: DEFAULT_STAGE_MAX_ACTORS,
        };
        assert_eq!(
            ids(&on_stage(&world(), &player(), None, &config)),
            ["away", "far", "mid", "near"]
        );
    }

    #[test]
    fn the_gate_admits_by_variant() {
        let stage: BTreeSet<ActorId> = [ActorId::from_raw("near")].into_iter().collect();
        let near = ActorId::from_raw("near");
        let away = ActorId::from_raw("away");

        assert!(IdleGate::All.allows(&away));
        assert!(IdleGate::Stage(&stage).allows(&near));
        assert!(!IdleGate::Stage(&stage).allows(&away));
        assert!(!IdleGate::Suppressed.allows(&near));
        assert!(IdleGate::Suppressed.is_suppressed());
        assert!(!IdleGate::All.is_suppressed());
    }

    #[test]
    fn only_an_explicit_all_turns_the_gate_off() {
        assert_eq!(
            IdleCognitionMode::from_config("all"),
            IdleCognitionMode::All
        );
        assert_eq!(
            IdleCognitionMode::from_config(" ALL "),
            IdleCognitionMode::All
        );
        assert_eq!(
            IdleCognitionMode::from_config("stage"),
            IdleCognitionMode::Stage
        );
        assert_eq!(
            IdleCognitionMode::from_config("nonsense"),
            IdleCognitionMode::Stage
        );
    }
}
