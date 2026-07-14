//! Who is allowed to think, and when.
//!
//! The turn stream has always been a clock: the round robin handed out slots
//! whether or not anybody was there to notice. This module is the answer to the
//! rule a player would state — an NPC thinks because you are near them, because
//! you spoke to them, or because something happened to them, never merely
//! because their name came up in a rotation
//! (`features/gate_idle_cognition_on_proximity.md`).
//!
//! Proximity was only half the rule. Being near someone justifies keeping them
//! *simulated*; it does not justify asking them, every three seconds, whether
//! they have changed their mind about saying nothing. So an on-stage actor also
//! has to have **news** — [`Novelty`], the second gate
//! (`features/gate_idle_cognition_on_novelty.md`). A turn that ends in silence
//! changes nothing for anybody, which is exactly why it must not buy another
//! turn: that is the loop that pays to be told nothing happened.
//!
//! It is pure and cheap: [`on_stage`] is a distance query the engine recomputes
//! once per poll, [`Novelty`] is a hash comparison beside it, and [`IdleGate`]
//! is all the scheduler ever learns about either. Nothing here gates the lanes
//! that fire on a *real* event — an addressed `say`, a sound nudge, the player's
//! own speech. Those reach an ambient fishmonger in an empty field exactly as
//! they always did, and an aloof NPC with no news still answers you the instant
//! you speak to them.

use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    HEARING_RADIUS_M, ITEM_INTERACTION_RADIUS_M, character::Control, ids::ActorId, world::World,
};

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

/// How long an actor remembers having thought, once they are off the stage.
///
/// Without a lapse the record is immortal, and an NPC you walked away from and
/// came back to an hour later would find nothing had changed since he last
/// thought — because, for him, nothing had. He would never look up again.
///
/// With one, the arrival of a player who has actually been *gone* is news
/// again. It must be comfortably longer than the churn it exists to absorb (see
/// [`Novelty::observe`]), and it is the knob to turn if returning to a street
/// re-greets you too eagerly.
pub const NOVELTY_MEMORY_SECONDS: f64 = 60.0;

/// What each actor had already been told when their last prompt went out.
///
/// This is derived bookkeeping, not world state, and it deliberately does not
/// live in [`World`]: nothing in the simulation may branch on it, and a save
/// file must not carry it. The engine owns one and recomputes it beside
/// [`on_stage`], once per poll (D20).
#[derive(Debug, Clone, Default)]
pub struct Novelty {
    last_told: BTreeMap<ActorId, Memory>,
}

#[derive(Debug, Clone, Copy)]
struct Memory {
    /// [`context_hash`] as it stood the moment their prompt was submitted — the
    /// world exactly as that prompt showed it to them.
    context: u64,
    /// When we last cared about this actor: the last poll they were on stage, or
    /// the last turn they took. Entries lapse [`NOVELTY_MEMORY_SECONDS`] after
    /// that, which is also what keeps the map bounded by the cast the player has
    /// actually met.
    touched_at: f64,
}

impl Novelty {
    /// Refresh the stage's memories and drop whoever has been gone too long.
    ///
    /// Touching every on-stage actor each poll — rather than only those who take
    /// a turn — is what makes the lapse mean *absence* instead of *silence*.
    /// Somebody standing quietly beside you for ten minutes is not forgotten and
    /// re-greeted; he is simply someone with nothing to say.
    ///
    /// Called once per poll, before [`Self::has_news`].
    pub fn observe(&mut self, now: f64, stage: &BTreeSet<ActorId>) {
        for actor_id in stage {
            if let Some(memory) = self.last_told.get_mut(actor_id) {
                memory.touched_at = now;
            }
        }
        // A NaN `now` would make every comparison false and retain everything,
        // which is the safe direction: a stale memory costs nothing but a missed
        // greeting, where a wrongly-dropped one costs a prompt.
        self.last_told
            .retain(|_, memory| now - memory.touched_at <= NOVELTY_MEMORY_SECONDS);
    }

    /// Whether anything has happened to `actor_id` since they last thought.
    ///
    /// An actor with no memory at all has news by definition — they have never
    /// been told anything, so the world in front of them is entirely new. That
    /// is the one turn an arrival is worth.
    pub fn has_news(&self, world: &World, actor_id: &ActorId) -> bool {
        let Some(actor) = world.characters.get(actor_id) else {
            return false;
        };
        // Somebody spoke near them, a sound reached them, a system line landed.
        // The inbox needs no memory of its own: a turn is what drains it, so a
        // non-empty one is *by construction* something they have not yet been
        // shown — including anything that arrived while their own prompt was in
        // flight.
        if !actor.inbox().is_empty() {
            return true;
        }
        match self.last_told.get(actor_id) {
            None => true,
            Some(memory) => memory.context != context_hash(world, actor_id),
        }
    }

    /// Record that `actor_id`'s prompt has gone out, showing them the world as it
    /// stands. Everything that happens after this instant is news.
    ///
    /// Submission — not completion — is the honest moment: the render has just
    /// drained their inbox, and a line that lands during the two seconds their
    /// call is in flight must survive to be shown on their *next* turn. Stamping
    /// on completion would swallow it.
    pub fn told(&mut self, now: f64, world: &World, actor_id: &ActorId) {
        self.last_told.insert(
            actor_id.clone(),
            Memory {
                context: context_hash(world, actor_id),
                touched_at: now,
            },
        );
    }
}

/// Everything an actor could react to that is not already in their inbox: who is
/// around them, and what is being offered.
///
/// The two rules that make this a gate rather than theatre:
///
/// - **Ids, never positions.** A neighbour's every step would otherwise be news,
///   and the player breathing on the spot would re-fire the whole stage.
///   `characters_within` orders by *distance*, so the ids are re-collected into
///   a [`BTreeSet`] first — otherwise two people merely swapping places would
///   reorder the hash and read as an arrival.
/// - **Their perspective, not the world's.** This mirrors the `you_see` and
///   `you_offer` / `offered_to_you` sections of the prompt, because those are
///   the only things a turn would show them. What it omits is as deliberate: the
///   clock, their own goal, their own inventory, their own `wait`. An actor is
///   not news to themselves.
pub fn context_hash(world: &World, actor_id: &ActorId) -> u64 {
    let mut hasher = DefaultHasher::new();
    let Some(actor) = world.characters.get(actor_id) else {
        return hasher.finish();
    };

    let seen: BTreeSet<ActorId> = world
        .characters_within(actor.position_m(), HEARING_RADIUS_M, Some(actor_id))
        .into_iter()
        .collect();
    for other_id in &seen {
        other_id.as_str().hash(&mut hasher);
    }

    // `world.offers` is keyed by item id and iterates in that order, so this is
    // stable without a sort. An offer of an item that has left the world shows
    // in neither prompt section, so it must not show here either.
    for offer in world.offers.values() {
        if !world.items.contains_key(&offer.item_id) {
            continue;
        }
        if offer.giver_id == *actor_id {
            // `you_offer`: what they have put on the table, and to whom.
            offer.item_id.as_str().hash(&mut hasher);
            offer
                .target_id
                .as_ref()
                .map(ActorId::as_str)
                .hash(&mut hasher);
        } else if offer.target_id.is_none() || offer.target_id.as_ref() == Some(actor_id) {
            // `offered_to_you`, which the prompt drops beyond arm's reach — so
            // an offer walking into range is news, exactly as it reads.
            let Some(giver) = world.characters.get(&offer.giver_id) else {
                continue;
            };
            if actor.position_m().distance_squared(giver.position_m())
                <= ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
            {
                offer.item_id.as_str().hash(&mut hasher);
                offer.giver_id.as_str().hash(&mut hasher);
            }
        }
    }

    hasher.finish()
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

    /// The load-bearing rule. If novelty keys on coordinates, the player
    /// breathing on the spot re-fires the whole stage and the feature is theatre.
    #[test]
    fn shuffling_on_the_spot_is_not_news() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let before = context_hash(&world, &near);

        // Everyone jiggles, nobody arrives or leaves. `characters_within` orders
        // by distance, so `mid` and `far` also *swap places* here — the case that
        // would reorder a naively-hashed list and read as an arrival.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(1.0, 0.0, 0.0);
        world
            .characters
            .get_mut(&ActorId::from_raw("mid"))
            .unwrap()
            .state
            .position_m = Vec3::new(18.0, 0.0, 0.0);
        world
            .characters
            .get_mut(&ActorId::from_raw("far"))
            .unwrap()
            .state
            .position_m = Vec3::new(17.0, 0.0, 0.0);

        assert_eq!(context_hash(&world, &near), before);
    }

    /// …and the other half of the same rule: a *body* arriving or leaving is.
    #[test]
    fn somebody_arriving_or_leaving_is_news() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let with_player = context_hash(&world, &near);

        // The player walks out of `near`'s 20 m hearing radius.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(500.0, 0.0, 0.0);
        let without_player = context_hash(&world, &near);
        assert_ne!(without_player, with_player);

        // And back. The world looks exactly as it did, so the hash says so —
        // this is what `Novelty`'s lapse exists to override.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(context_hash(&world, &near), with_player);
    }

    #[test]
    fn an_inbox_is_news_all_by_itself() {
        let world = world();
        let near = ActorId::from_raw("near");
        let mut novelty = Novelty::default();

        // Never thought: the world in front of them is entirely new.
        assert!(novelty.has_news(&world, &near));

        novelty.told(0.0, &world, &near);
        assert!(!novelty.has_news(&world, &near));

        // Somebody speaks. Nobody moved, so the context hash is untouched — the
        // inbox is the whole signal, and it needs no memory of its own because a
        // turn is what drains it.
        let mut world = world;
        world
            .characters
            .get_mut(&near)
            .unwrap()
            .notify_percept("Someone said: \"Hoy!\"".to_string());
        assert!(novelty.has_news(&world, &near));
    }

    /// A turn that ends in silence must not buy another turn. This is the loop
    /// the whole feature exists to break.
    #[test]
    fn silence_stays_silent() {
        let world = world();
        let near = ActorId::from_raw("near");
        let mut novelty = Novelty::default();

        novelty.told(0.0, &world, &near);
        // A `wait` emits no domain event and no percept, so nothing about the
        // world has changed — and nothing keeps changing, poll after poll.
        for tick in 0..100 {
            novelty.observe(tick as f64 * 0.1, &[near.clone()].into_iter().collect());
            assert!(!novelty.has_news(&world, &near), "re-fired at tick {tick}");
        }
    }

    /// Standing quietly beside the player for ten minutes is not absence. If the
    /// lapse counted silence rather than absence, the gate would leak a turn per
    /// actor per minute — exactly the bill it is meant to remove.
    #[test]
    fn a_quiet_neighbour_is_never_forgotten() {
        let world = world();
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let mut novelty = Novelty::default();
        novelty.told(0.0, &world, &near);

        let mut now = 0.0;
        while now < 600.0 {
            now += 1.0;
            novelty.observe(now, &stage);
        }
        assert!(!novelty.has_news(&world, &near));
    }

    /// But a real absence is. Walk away, come back, and he looks up — even though
    /// the street he last thought about is exactly the street he sees now.
    ///
    /// This is also what absorbs the churn of the `max_actors` cap: a neighbour
    /// bumped off the stage for a second by somebody closer keeps his memory, and
    /// does not re-greet you for the privilege of coming back.
    #[test]
    fn an_actor_the_player_left_forgets_they_ever_met() {
        let world = world();
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let empty = BTreeSet::new();
        let mut novelty = Novelty::default();
        novelty.told(0.0, &world, &near);

        // Off stage, but only briefly — the cap churned, or the player stepped
        // over the radius and back.
        novelty.observe(NOVELTY_MEMORY_SECONDS - 1.0, &empty);
        novelty.observe(NOVELTY_MEMORY_SECONDS - 1.0, &stage);
        assert!(!novelty.has_news(&world, &near));

        // Genuinely gone. The memory lapses, and the next arrival is worth a turn.
        novelty.observe(NOVELTY_MEMORY_SECONDS * 3.0, &empty);
        assert!(novelty.has_news(&world, &near));
    }

    #[test]
    fn an_offer_is_news_and_an_unknown_actor_never_is() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let before = context_hash(&world, &near);

        // `mid` is 10 m away — well outside the 4 m the prompt will show an
        // offer at, so it is not yet `near`'s business.
        let coin = crate::ids::ItemId::from_raw("c0prs");
        world
            .items
            .insert(coin.clone(), crate::item::Item::new(coin.clone(), "a coin"));
        world.offers.insert(
            coin.clone(),
            crate::offer::Offer {
                item_id: coin,
                giver_id: ActorId::from_raw("mid"),
                target_id: Some(near.clone()),
                created_seq: 1,
            },
        );
        assert_eq!(context_hash(&world, &near), before);

        // The giver walks into arm's reach: now it appears in `offered_to_you`,
        // and an offer appearing in front of you is news.
        world
            .characters
            .get_mut(&ActorId::from_raw("mid"))
            .unwrap()
            .state
            .position_m = Vec3::new(6.0, 0.0, 0.0);
        assert_ne!(context_hash(&world, &near), before);

        // A hash for somebody who is not in the world is a question with no
        // answer, and must not panic.
        let novelty = Novelty::default();
        assert!(!novelty.has_news(&world, &ActorId::from_raw("nobody")));
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
