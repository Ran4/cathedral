//! The water round (M3) — the first vertical slice of the movement feature's
//! non-LLM behaviour layer (`features/movement/03_the_ladder.md`,
//! [README §8](../../../features/movement/README.md)).
//!
//! It is the whole stack, end to end and for zero tokens: the clock decays a
//! **thirst** need, a flat first-match-wins **ladder** (rungs 2, 6, 11, 12)
//! turns that need into a **route** to a public well, a **queue** forms at the
//! curb with household vessels going before trade ones, the drawer's turn
//! **draws** water (thirst refilled) while the keeper works the gear and the
//! windlass is *heard* — a percept the LLM layer can be asked about — and then
//! the drawer walks **home again**.
//!
//! All state lives on the [`Engine`](crate::Engine) (a [`WaterRound`]), driven
//! once per poll right after the movement tick. It is pure and deterministic:
//! every "random" choice is a hash of `(actor_id, epoch, salt)`, never an RNG,
//! following the `attention.rs` curiosity idiom the ladder doc points at. With
//! no nav graph the whole thing no-ops, so the frozen golden fixtures are
//! untouched.

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    LADDER_DECISION_MAX_SECONDS, LADDER_DECISION_MIN_SECONDS, SOCIAL_PULL_RADIUS_M, THIRST_MAX,
    THIRST_PARCHED, THIRST_THIRSTY, WALK_SPEED_MPS, WANDER_LEASH_M, WATER_DRAW_SECONDS,
    WELL_ARRIVE_RADIUS_M, WELL_KEEPER_SOUND_INTERVAL_SECONDS, WELL_QUEUE_SHORT,
    character::{Character, Movement},
    clock::WorldClock,
    event::DomainEvent,
    ids::ActorId,
    lore::Significance,
    math::Vec3,
    nav::{NavData, WALK_Y},
    world::{World, planar_close},
};

/// The M2 pacer, excluded from the water round so it keeps its own ping-pong.
const PACING_ACTOR_ID: &str = "p0012";
/// Occupations that fetch water with a **household** vessel — chiefly the
/// servant, whose day is a trip to the ward well and back (the single largest
/// occupation in the city, `features/movement/README.md` §8). A household vessel
/// takes precedence in the queue (`lore/wells_and_water.md`).
const HOUSEHOLD_OCCUPATIONS: &[&str] = &["domestic_servant"];
/// Occupations that draw with a **trade** vessel and so queue *behind* the
/// households — the fullers, dyers and cloth-washers the cisterns were assessed
/// for (`lore/wells_and_water.md`: Tenter Cistern's "fullers, cloth washers,
/// dyers").
const TRADE_OCCUPATIONS: &[&str] = &["cloth_worker", "garment_worker"];

/// Whether an occupation fetches water, and with what vessel: `Some(true)` is a
/// household vessel, `Some(false)` a trade one, `None` is not a drawer.
fn vessel_of(occupation: Option<&str>) -> Option<bool> {
    match occupation {
        Some(occupation) if HOUSEHOLD_OCCUPATIONS.contains(&occupation) => Some(true),
        Some(occupation) if TRADE_OCCUPATIONS.contains(&occupation) => Some(false),
        _ => None,
    }
}
/// A source only takes a keeper from an NPC standing within this of its curb, so
/// nobody is dragged across the city to work a well.
const KEEPER_MAX_DIST_M: f64 = 55.0;

/// The **nine** public drinking sources (`07_milestones.md` M3), by their
/// nav-graph display name (the nav places are keyed by name, not by the
/// `chain_well` area id), each paired with the sound its gear makes: the deep
/// chain well clanks its windlass, the rope wells break a bucket on the water far
/// below, the roof-fed cisterns pour a pail into a trough. `lore/wells_and_water.md`
/// names exactly these nine as household water and puts **both** the Shambles
/// well (a slaughter-yard *work* well — "not a preferred household source") and
/// `Seven Lofts fire tanks` (a fire reserve) outside the ward drinking list, so
/// both are deliberately absent here.
const SOURCES: &[(&str, &str)] = &[
    ("Ford Well", "draw_water"),
    ("Chain Well", "chain_windlass"),
    ("Bitter Well", "draw_water"),
    ("Three-Curb", "draw_water"),
    ("Lodge Well", "draw_water"),
    ("Slate Cistern", "pour_trough"),
    ("Tenter Cistern", "pour_trough"),
    ("Reed Cistern", "pour_trough"),
    ("Step Cistern", "pour_trough"),
];

/// One public well or cistern: where its water is drawn, what it sounds like, who
/// keeps it, and the queue waiting at the curb.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterSource {
    /// The nav place display name, e.g. `"Chain Well"`.
    pub name: String,
    /// The walkable node the queue forms at and the sound comes from.
    pub draw_point: Vec3,
    /// The catalog sound the keeper makes working the gear.
    pub draw_sound: &'static str,
    /// The actor who works the curb, or `None` for a source with nobody nearby
    /// to keep it (which then draws no water and makes no sound).
    pub keeper: Option<ActorId>,
    /// Ordered turns: the front draws next. Household vessels are inserted ahead
    /// of trade vessels (`lore/wells_and_water.md`), preserving arrival order
    /// within each class.
    pub queue: Vec<ActorId>,
    /// The drawer currently at the curb and the real-clock time their draw ends.
    serving: Option<(ActorId, f64)>,
    /// Next real-clock time the keeper works the gear while the source is busy.
    keeper_next_sound: f64,
}

/// Where an enrolled drawer is in its water errand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// At or near home, milling about; the ladder may send them to a well.
    Idle,
    /// Walking to their assigned well.
    Approaching,
    /// Standing in the queue at the curb.
    Queued,
    /// At the front, drawing.
    Drawing,
    /// Walking home again after drawing.
    Returning,
}

/// One person who fetches water: where home is, which source they use, whether
/// their vessel takes household precedence, and where they are in the errand.
#[derive(Debug, Clone, PartialEq)]
struct Waterer {
    home: Vec3,
    source: usize,
    is_household: bool,
    phase: Phase,
    /// Real-clock time of this idle actor's next ladder evaluation.
    next_decision: f64,
    /// Bumped each decision; the salt that makes the deterministic choices vary.
    epoch: u64,
}

/// The whole water round: the staffed sources and everyone who draws from them.
/// Inert until [`WaterRound::seed`] runs (only when the host supplies a nav
/// graph), so a world without nav has an empty round and no behaviour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterRound {
    sources: Vec<WaterSource>,
    waterers: BTreeMap<ActorId, Waterer>,
    /// Game-days at the last tick, so thirst decays by the game clock (and speeds
    /// up with the debug time-scale), not by wall-clock.
    last_game_days: f64,
    seeded: bool,
}

impl WaterRound {
    pub fn new() -> Self {
        Self::default()
    }

    /// The staffed sources, for diagnostics and tests.
    pub fn sources(&self) -> &[WaterSource] {
        &self.sources
    }

    /// The current queue at a named source, front first — for tests/tracing.
    pub fn queue_at(&self, name: &str) -> Option<&[ActorId]> {
        self.sources
            .iter()
            .find(|source| source.name == name)
            .map(|source| source.queue.as_slice())
    }

    /// Whether a drawer is currently drawing at a named source — for tests.
    pub fn is_drawing_at(&self, name: &str) -> bool {
        self.sources
            .iter()
            .find(|source| source.name == name)
            .is_some_and(|source| source.serving.is_some())
    }

    /// A one-line census of the round for `--trace-water`: how many sources are
    /// kept, how many drawers there are and how many are thirsty, and how many
    /// stand in a queue or at a curb right now.
    pub fn summary(&self, world: &World) -> String {
        let staffed = self.sources.iter().filter(|source| source.keeper.is_some()).count();
        let queued: usize = self.sources.iter().map(|source| source.queue.len()).sum();
        let drawing = self.sources.iter().filter(|source| source.serving.is_some()).count();
        let thirsty = self
            .waterers
            .keys()
            .filter(|id| {
                world
                    .characters
                    .get(*id)
                    .is_some_and(|character| character.needs().thirst < THIRST_THIRSTY)
            })
            .count();
        format!(
            "water: {} sources / {staffed} kept | {} drawers, {thirsty} thirsty | queued {queued}, drawing {drawing}",
            self.sources.len(),
            self.waterers.len(),
        )
    }

    /// Enrol the round: resolve each source's draw point from the nav graph, give
    /// each a keeper from the nearest idle NPC, and sign up every water-drawing
    /// character to their nearest staffed source. Returns one human line per
    /// thing that did not resolve, so the caller can log and carry on — it never
    /// panics and only touches the actors it enrols.
    pub fn seed(&mut self, world: &mut World, nav: &NavData, now: f64, clock: &WorldClock) -> Vec<String> {
        let mut diagnostics = Vec::new();
        if self.seeded {
            return diagnostics;
        }
        self.seeded = true;
        self.last_game_days = clock.game_days(now);

        // Resolve the sources present in this nav graph.
        for &(name, sound) in SOURCES {
            match nav.place(name) {
                Some(place) => self.sources.push(WaterSource {
                    name: name.to_string(),
                    draw_point: nav.node_point(place.node),
                    draw_sound: sound,
                    keeper: None,
                    queue: Vec::new(),
                    serving: None,
                    keeper_next_sound: now,
                }),
                None => diagnostics.push(format!(
                    "[smart actors] water round: nav place {name:?} is missing; source skipped"
                )),
            }
        }
        if self.sources.is_empty() {
            diagnostics.push("[smart actors] water round: no water sources resolved".to_string());
            return diagnostics;
        }

        // Every LLM townsperson except the player and the M2 pacer, in
        // deterministic roster order — the pool drawers come from (any
        // significance, so the one minor servant and any minor fullers still
        // fetch water).
        let townsfolk: Vec<ActorId> = world
            .roster
            .iter()
            .filter(|id| {
                id.as_str() != PACING_ACTOR_ID
                    && world
                        .characters
                        .get(*id)
                        .is_some_and(|character| character.control().is_llm())
            })
            .cloned()
            .collect();

        // A keeper each: the nearest free **ambient** townsperson to the curb,
        // pinned there so a queue has someone to form on. Restricting keepers to
        // ambients means pinning one never overrides a named character's authored
        // place; a source with no ambient nearby simply goes unkept, which is fine.
        let mut used: std::collections::BTreeSet<ActorId> = std::collections::BTreeSet::new();
        for index in 0..self.sources.len() {
            let draw_point = self.sources[index].draw_point;
            let keeper = townsfolk
                .iter()
                .filter(|id| {
                    !used.contains(*id)
                        && world.characters[*id].significance() == Significance::Ambient
                })
                .filter_map(|id| {
                    let distance = world.characters[id].position_m().distance(draw_point);
                    (distance <= KEEPER_MAX_DIST_M).then_some((distance, id))
                })
                .min_by(|left, right| {
                    left.0
                        .partial_cmp(&right.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.1.cmp(right.1))
                })
                .map(|(_, id)| id.clone());
            if let Some(keeper) = keeper {
                used.insert(keeper.clone());
                // Stand the keeper at the curb (a keeper works their well). Set
                // before the first snapshot, so it is a spawn, not a teleport.
                world.characters.get_mut(&keeper).expect("keeper exists").state.position_m = draw_point;
                self.sources[index].keeper = Some(keeper);
            }
        }

        // Everyone who draws water: bound to the nearest *staffed* source, home is
        // their spawn, thirst spread across the range so the curbs get busy at
        // once instead of all at the same later moment.
        let staffed: Vec<usize> = (0..self.sources.len())
            .filter(|index| self.sources[*index].keeper.is_some())
            .collect();
        if staffed.is_empty() {
            diagnostics
                .push("[smart actors] water round: no source could be staffed".to_string());
            return diagnostics;
        }

        let mut enrolled = 0usize;
        for id in &townsfolk {
            if used.contains(id) {
                continue; // a keeper does not also queue at their own well
            }
            let occupation = world.characters[id]
                .lore()
                .and_then(|lore| lore.occupation_id.as_deref());
            let Some(is_household) = vessel_of(occupation) else {
                continue;
            };
            let home = world.characters[id].position_m();
            let source = *staffed
                .iter()
                .min_by(|left, right| {
                    let dl = self.sources[**left].draw_point.distance(home);
                    let dr = self.sources[**right].draw_point.distance(home);
                    dl.partial_cmp(&dr).unwrap_or(std::cmp::Ordering::Equal)
                })
                .expect("staffed is non-empty");

            let thirst = THIRST_MAX * hash01("water_thirst_seed", id, 0);
            world.characters.get_mut(id).expect("drawer exists").state.needs.thirst = thirst;
            self.waterers.insert(
                id.clone(),
                Waterer {
                    home,
                    source,
                    is_household,
                    phase: Phase::Idle,
                    next_decision: now + decision_jitter(id, 0),
                    epoch: 0,
                },
            );
            enrolled += 1;
        }

        diagnostics.push(format!(
            "[smart actors] water round: {} sources, {} staffed, {enrolled} drawers",
            self.sources.len(),
            staffed.len(),
        ));
        diagnostics
    }
}

/// Advance the round one poll. Split from the [`WaterRound`] methods as a free
/// function so it can borrow the round and the world (disjoint [`Engine`] fields)
/// at once. A no-op until [`WaterRound::seed`] has run. `player_id` is who the
/// well sounds play *for*: they are player-audible ambience, never an NPC percept
/// or a reaction nudge (see [`service_sources`]).
pub fn tick(
    round: &mut WaterRound,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    player_id: &ActorId,
) {
    if round.sources.is_empty() {
        return;
    }

    decay_thirst(round, world, clock, now);
    resolve_arrivals(round, world);
    service_sources(round, world, nav, now, player_id);
    run_ladder(round, world, nav, now);
}

/// Thirst falls by the game clock so it keeps pace with the sun and the debug
/// time-scale. Only enrolled drawers decay — the rest of the cast has no need
/// gauge in play until M4.
fn decay_thirst(round: &mut WaterRound, world: &mut World, clock: &WorldClock, now: f64) {
    let game_days = clock.game_days(now);
    let delta_days = (game_days - round.last_game_days).max(0.0);
    round.last_game_days = game_days;
    let drop = delta_days * 86_400.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
    if drop <= 0.0 {
        return;
    }
    for id in round.waterers.keys() {
        if let Some(character) = world.characters.get_mut(id) {
            character.state.needs.thirst = (character.state.needs.thirst - drop).max(0.0);
        }
    }
}

/// Move arrivals through the state machine: an approacher who has reached the
/// curb joins the queue; a returner who has reached home falls idle again.
fn resolve_arrivals(round: &mut WaterRound, world: &mut World) {
    let ids: Vec<ActorId> = round.waterers.keys().cloned().collect();
    for id in ids {
        let (phase, source_idx) = {
            let waterer = &round.waterers[&id];
            (waterer.phase, waterer.source)
        };
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        if character.is_walking() {
            continue; // still on the way
        }
        let position = character.position_m();

        match phase {
            Phase::Approaching => {
                let at_curb =
                    position.distance(round.sources[source_idx].draw_point) <= WELL_ARRIVE_RADIUS_M;
                if at_curb {
                    enqueue(round, source_idx, id.clone());
                    round.waterers.get_mut(&id).expect("waterer exists").phase = Phase::Queued;
                    // Stop being a mover while queued.
                    world.characters.get_mut(&id).expect("drawer exists").state.movement = None;
                } else {
                    // The route stopped short (blocked or already close-but-not
                    // arrived); re-decide next cadence rather than stranding them.
                    round.waterers.get_mut(&id).expect("waterer exists").phase = Phase::Idle;
                }
            }
            Phase::Returning => {
                round.waterers.get_mut(&id).expect("waterer exists").phase = Phase::Idle;
                world.characters.get_mut(&id).expect("drawer exists").state.movement = None;
            }
            _ => {}
        }
    }
}

/// Insert `actor` into the source's queue, household vessels ahead of trade ones,
/// preserving arrival order within each class (`lore/wells_and_water.md`).
fn enqueue(round: &mut WaterRound, source_idx: usize, actor: ActorId) {
    let is_household = round.waterers[&actor].is_household;
    if is_household {
        let insert_at = round.sources[source_idx]
            .queue
            .iter()
            .position(|id| !round.waterers.get(id).is_some_and(|waterer| waterer.is_household))
            .unwrap_or(round.sources[source_idx].queue.len());
        round.sources[source_idx].queue.insert(insert_at, actor);
    } else {
        round.sources[source_idx].queue.push(actor);
    }
}

/// Work each curb: finish a completed draw (refill, remember it, send home),
/// start the next one, and clank the gear while anybody is waiting.
///
/// **The well sound is a clock, not an event** (`features/movement/05_the_llm_seam.md`
/// §5.2, exactly as the bell is): it is emitted as an *unattributed world sound
/// heard only by the player*, so it reaches no NPC inbox and — critically — never
/// nudges a reaction turn. Ten curbs clanking every few seconds would otherwise
/// pin the scheduler's single priority slot forever (`scheduler.rs`: a nudge is
/// ungated by proximity) and burn the token budget the round is meant to cost
/// nothing. The drawer instead *remembers their own draw*, so the person the
/// player walks up to and asks can still say what they are doing (README §8).
fn service_sources(round: &mut WaterRound, world: &mut World, nav: &NavData, now: f64, player_id: &ActorId) {
    let mut finished: Vec<ActorId> = Vec::new();
    let mut started: Vec<ActorId> = Vec::new();
    let mut emissions: Vec<(&'static str, Vec3)> = Vec::new();

    for source in &mut round.sources {
        // Finish the current draw.
        if let Some((drawer, ends_at)) = source.serving.clone()
            && now >= ends_at
        {
            source.serving = None;
            if source.queue.first() == Some(&drawer) {
                source.queue.remove(0);
            } else {
                source.queue.retain(|id| id != &drawer);
            }
            finished.push(drawer);
        }
        // Start the next one, if the well is kept and somebody is waiting.
        if source.serving.is_none()
            && source.keeper.is_some()
            && let Some(front) = source.queue.first().cloned()
        {
            source.serving = Some((front.clone(), now + WATER_DRAW_SECONDS));
            started.push(front);
        }
        // Keep the gear turning while the source is busy.
        let busy = source.serving.is_some() || !source.queue.is_empty();
        if busy
            && source.keeper.is_some()
            && now >= source.keeper_next_sound
        {
            source.keeper_next_sound = now + WELL_KEEPER_SOUND_INTERVAL_SECONDS;
            emissions.push((source.draw_sound, source.draw_point));
        }
    }

    for drawer in started {
        // The memory of drawing lands as the turn *begins*, so the person now at
        // the front — the one the player walks up to and asks — can already say
        // what they are doing (README §8). No sound broadcast, no nudge.
        let self_line = round
            .waterers
            .get(&drawer)
            .map(|waterer| round.sources[waterer.source].draw_sound)
            .and_then(|sound_id| world.sound_catalog.get(sound_id))
            .and_then(|sound| sound.seen.as_ref())
            .map(|seen| seen.replace("{actor}", "You"));
        if let Some(waterer) = round.waterers.get_mut(&drawer) {
            waterer.phase = Phase::Drawing;
        }
        if let (Some(line), Some(character)) = (self_line, world.characters.get_mut(&drawer)) {
            character.remember_percept(line);
        }
    }
    for drawer in finished {
        if let Some(character) = world.characters.get_mut(&drawer) {
            character.state.needs.thirst = THIRST_MAX; // a full vessel
        }
        // Walk home again; the ladder resumes there.
        let home = round.waterers.get(&drawer).map(|waterer| waterer.home);
        let position = world.characters.get(&drawer).map(Character::position_m);
        if let (Some(home), Some(position)) = (home, position) {
            match route_path(nav, position, home) {
                Some(path) => set_route(world, &drawer, path),
                None => {
                    world.characters.get_mut(&drawer).expect("drawer exists").state.movement = None;
                }
            }
        }
        if let Some(waterer) = round.waterers.get_mut(&drawer) {
            waterer.phase = Phase::Returning;
        }
    }
    // The player-audible ambient: an unattributed world sound at the curb, heard
    // only if the player is within range — no NPC recipient, so no reaction nudge.
    let player_pos = world.characters.get(player_id).map(Character::position_m);
    for (sound_id, position) in emissions {
        if !world.sounds_enabled {
            continue;
        }
        let Some(sound) = world.sound_catalog.get(sound_id).cloned() else {
            continue;
        };
        let recipients = match player_pos {
            Some(pos) if pos.distance(position) <= sound.audible_distance => vec![player_id.clone()],
            _ => Vec::new(),
        };
        world.emit(DomainEvent::sound(
            sound.sound_class.clone(),
            None, // a world sound, like the bell — never attributed, never nudges
            sound.sound_id.clone(),
            sound.audible_distance,
            position,
            recipients,
            Vec::new(),
        ));
    }
}

/// The behaviour ladder, run for idle drawers whose walk has ended and whose
/// cadence has come round. First match wins (`features/movement/03_the_ladder.md`
/// §4); M3 ships rungs 2, 6, 11 and 12.
fn run_ladder(round: &mut WaterRound, world: &mut World, nav: &NavData, now: f64) {
    let ids: Vec<ActorId> = round.waterers.keys().cloned().collect();
    for id in ids {
        let (phase, epoch, source_idx, home, next_decision) = {
            let waterer = &round.waterers[&id];
            (waterer.phase, waterer.epoch, waterer.source, waterer.home, waterer.next_decision)
        };
        if phase != Phase::Idle {
            continue; // committed to a well trip; nothing re-decides mid-errand
        }
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        if character.is_walking() || now < next_decision {
            continue;
        }

        let position = character.position_m();
        let thirst = character.needs().thirst;
        let queue_len = round.sources[source_idx].queue.len();
        let decision = decide(world, nav, &id, epoch, position, home, thirst, queue_len);

        // Schedule the next evaluation and advance the salt.
        {
            let waterer = round.waterers.get_mut(&id).expect("waterer exists");
            waterer.epoch = epoch.wrapping_add(1);
            waterer.next_decision = now + decision_jitter(&id, waterer.epoch);
        }

        match decision {
            Decision::Approach => {
                let draw_point = round.sources[source_idx].draw_point;
                match route_path(nav, position, draw_point) {
                    Some(path) => {
                        set_route(world, &id, path);
                        round.waterers.get_mut(&id).expect("waterer exists").phase = Phase::Approaching;
                    }
                    // Already standing at the curb: join the queue now.
                    None => {
                        enqueue(round, source_idx, id.clone());
                        round.waterers.get_mut(&id).expect("waterer exists").phase = Phase::Queued;
                        world.characters.get_mut(&id).expect("drawer exists").state.movement = None;
                    }
                }
            }
            Decision::Wander(target) => {
                if let Some(path) = route_path(nav, position, target) {
                    set_route(world, &id, path);
                }
            }
            Decision::Stay => {}
        }
    }
}

/// One idle drawer's rung, read against the four sources of truth
/// (`features/movement/03_the_ladder.md` §2).
enum Decision {
    /// Rungs 2 & 6: set off for the assigned well.
    Approach,
    /// Rungs 11 & 12: drift toward a friend, or mill about near home.
    Wander(Vec3),
    /// Stand where you are this cadence.
    Stay,
}

#[allow(clippy::too_many_arguments)]
fn decide(
    world: &World,
    nav: &NavData,
    id: &ActorId,
    epoch: u64,
    position: Vec3,
    home: Vec3,
    thirst: f64,
    queue_len: usize,
) -> Decision {
    // Rung 2 — parched: the well, now, whatever the queue.
    if thirst < THIRST_PARCHED {
        return Decision::Approach;
    }
    // Rung 6 — thirsty: the well, but only if its queue is short.
    if thirst < THIRST_THIRSTY && queue_len < WELL_QUEUE_SHORT {
        return Decision::Approach;
    }
    // Rung 11 — the social pull: drift toward a known, settled neighbour so the
    // people the player walks up to are already a scene.
    if let Some(friend) = nearest_known_settled(world, id, position) {
        let toward = drift_target(home, position, friend);
        if nav.is_walkable(toward.x, toward.z) {
            return Decision::Wander(toward);
        }
    }
    // Rung 12 — wander: mill about within a leash of home, but not every time —
    // people mostly stand.
    if hash01("water_wander_gate", id, epoch) < 0.4
        && let Some(target) = wander_target(nav, id, epoch, home)
    {
        return Decision::Wander(target);
    }
    Decision::Stay
}

/// The nearest LLM neighbour within the social radius that this actor knows by
/// name and that has stopped moving.
fn nearest_known_settled(world: &World, id: &ActorId, position: Vec3) -> Option<Vec3> {
    let me = world.characters.get(id)?;
    for neighbour_id in world.characters_within(position, SOCIAL_PULL_RADIUS_M, Some(id)) {
        let neighbour = &world.characters[&neighbour_id];
        if neighbour.control().is_llm() && neighbour.is_settled() && me.knows().contains(&neighbour_id)
        {
            return Some(neighbour.position_m());
        }
    }
    None
}

/// A point a stride short of `friend`, clamped to the leash around `home`, so the
/// drift reads as "coming over" without piling onto them.
fn drift_target(home: Vec3, position: Vec3, friend: Vec3) -> Vec3 {
    let toward = friend - position;
    let length = toward.length();
    let target = if length > 1.5 {
        position + toward / length * (length - 1.0)
    } else {
        position
    };
    clamp_to_leash(home, target)
}

/// A deterministic walkable point within [`WANDER_LEASH_M`] of home, or `None` if
/// a few hashed tries all land on stone.
fn wander_target(nav: &NavData, id: &ActorId, epoch: u64, home: Vec3) -> Option<Vec3> {
    for attempt in 0..4 {
        let angle = hash01("water_wander_angle", id, epoch ^ (attempt as u64)) * std::f64::consts::TAU;
        let radius = hash01("water_wander_radius", id, epoch.wrapping_add(attempt as u64)) * WANDER_LEASH_M;
        let target = Vec3::new(home.x + angle.cos() * radius, WALK_Y, home.z + angle.sin() * radius);
        if nav.is_walkable(target.x, target.z) {
            return Some(target);
        }
    }
    None
}

/// Pull `target` back to the edge of the leash circle around `home` if it strays.
fn clamp_to_leash(home: Vec3, target: Vec3) -> Vec3 {
    let offset = Vec3::new(target.x - home.x, 0.0, target.z - home.z);
    let length = offset.length();
    if length <= WANDER_LEASH_M {
        Vec3::new(target.x, WALK_Y, target.z)
    } else {
        let edge = offset / length * WANDER_LEASH_M;
        Vec3::new(home.x + edge.x, WALK_Y, home.z + edge.z)
    }
}

/// Route `from` → `to` and trim the leading node when it is where we already
/// stand (else the first leg is zero-length). `None` means already there or no
/// route — the same discipline `seed_pacing_actor` uses.
fn route_path(nav: &NavData, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    let route = nav.route_between(from, to)?;
    let mut path = route.points;
    if path.first().is_some_and(|point| planar_close(*point, from)) {
        path.remove(0);
    }
    if path.is_empty() { None } else { Some(path) }
}

/// Give a drawer a fresh walk with no patrol, keeping their gait phase seamless.
fn set_route(world: &mut World, id: &ActorId, path: Vec<Vec3>) {
    let Some(character) = world.characters.get_mut(id) else {
        return;
    };
    let gait_phase = character
        .state
        .movement
        .as_ref()
        .map_or(0.0, |movement| movement.gait_phase);
    character.state.movement = Some(Movement {
        path,
        speed: WALK_SPEED_MPS,
        gait_phase,
        patrol: None,
    });
}

/// A jittered 1–6 s cadence (real seconds), staggered across the cast by the
/// per-actor hash so the ladder needs no scheduler.
fn decision_jitter(id: &ActorId, epoch: u64) -> f64 {
    LADDER_DECISION_MIN_SECONDS
        + (LADDER_DECISION_MAX_SECONDS - LADDER_DECISION_MIN_SECONDS)
            * hash01("water_decision", id, epoch)
}

/// A pure `[0, 1)` roll from `(salt, actor_id, epoch)` — the sim's deterministic
/// stand-in for an RNG (`attention.rs` curiosity idiom; the sim has no clock and
/// no randomness by decree).
fn hash01(salt: &str, id: &ActorId, epoch: u64) -> f64 {
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    id.as_str().hash(&mut hasher);
    epoch.hash(&mut hasher);
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Office, WorldClock,
        character::{CharacterSheet, Control},
        event::EventType,
        lore::{LoreProfile, PlanningWard, Significance},
        sounds::SoundCatalog,
    };
    use std::collections::BTreeSet;

    const NAV_JSON: &str = include_str!("../../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../../assets/world/navigation.bin");
    const CATALOG: &str = include_str!("../../../assets/sounds/catalog.toml");

    fn nav() -> NavData {
        NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav loads")
    }

    fn clock() -> WorldClock {
        // One game day per real hour, opening at Dayspring — the shipped default.
        WorldClock::new(3600.0, Office::Dayspring, 0, 0.05)
    }

    /// A character at `position`, optionally a `domestic_servant` (the occupation
    /// the round enrols) — otherwise a plain LLM townsperson (a keeper candidate).
    fn person(id: &str, position: Vec3, occupation: Option<&str>) -> Character {
        let lore = occupation.map(|occupation_id| LoreProfile {
            significance: Significance::Ambient,
            planning_ward: PlanningWard::Fabric,
            age: 30,
            gender: "f".into(),
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
            core_character_description: String::new(),
            extended_character_description: String::new(),
            curiosity: None,
        });
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control: Control::Llm,
            back_story: String::new(),
            location_description: String::new(),
            appearance_key: id.into(),
            voice_key: None,
            position_m: position,
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore,
        })
    }

    fn base_world() -> World {
        let mut world = World::new();
        world.sound_catalog = SoundCatalog::from_toml_str(CATALOG).expect("catalog loads");
        world
    }

    fn player() -> ActorId {
        ActorId::from_raw("player")
    }

    /// One engine-style beat: walk the movers a slice, then run the round.
    fn beat(round: &mut WaterRound, world: &mut World, nav: &NavData, clock: &WorldClock, now: f64, dt: f64) {
        world.step_movement(dt, nav);
        tick(round, world, nav, clock, now, &player());
    }

    /// The vertical slice: a parched servant walks to the well, queues, draws
    /// (thirst refilled, the windlass heard), then heads home again.
    #[test]
    fn a_parched_servant_walks_to_the_well_draws_and_goes_home() {
        let nav = nav();
        let clock = clock();
        let ford = nav.place("Ford Well").expect("Ford Well is a nav place").node;
        let curb = nav.node_point(ford);
        // A short, guaranteed route: start the servant one graph hop from the well.
        let hop = nav.adjacency()[ford]
            .first()
            .expect("the well node has a neighbour")
            .to;
        let home = nav.node_point(hop);

        let mut world = base_world();
        world.add_character(person("keeper", curb, Some("mason"))); // an ambient at the curb -> keeper
        world.add_character(person("servant", home, Some(HOUSEHOLD_OCCUPATIONS[0])));

        let mut round = WaterRound::new();
        let diagnostics = round.seed(&mut world, &nav, 0.0, &clock);
        assert!(
            round.sources().iter().any(|source| source.name == "Ford Well" && source.keeper.is_some()),
            "Ford Well was staffed: {diagnostics:?}"
        );
        // The keeper stands at the curb; the servant enrolled as a drawer.
        assert_eq!(world.characters[&ActorId::from_raw("keeper")].position_m(), curb);

        // Parch the servant so rung 2 fires at once.
        world
            .characters
            .get_mut(&ActorId::from_raw("servant"))
            .unwrap()
            .state
            .needs
            .thirst = 0.0;

        let servant = ActorId::from_raw("servant");
        let dt = 0.2;
        let mut now = 0.0;
        let mut drew = false;
        let mut drew_at_max = false;
        let mut windlass_events = 0;
        let mut remembered = false;
        let mut went_home = false;

        for _ in 0..3000 {
            now += dt;
            beat(&mut round, &mut world, &nav, &clock, now, dt);
            for event in world.drain_events() {
                // A world sound at the well: unattributed (no actor) so it never
                // nudges an NPC — the whole point of the M3 sound model.
                if event.event_type == EventType::Sound
                    && event.sound_id.as_deref() == Some("draw_water")
                {
                    windlass_events += 1;
                    assert!(event.actor_id.is_none(), "the windlass is a world sound, never attributed");
                    assert!(event.witness_ids.is_empty(), "a world sound has no witnesses to nudge");
                }
            }
            if round.is_drawing_at("Ford Well") {
                drew = true;
            }
            // The drawer remembers drawing, so the player can ask them.
            if world.characters[&servant]
                .recent_history()
                .iter()
                .any(|line| line.contains("drew water"))
            {
                remembered = true;
            }
            // Refilled by a completed draw.
            if drew && world.characters[&servant].needs().thirst >= THIRST_MAX - 1.0 {
                drew_at_max = true;
            }
            // Home again: back within a stride of where they started, after drawing.
            if drew_at_max
                && !world.characters[&servant].is_walking()
                && world.characters[&servant].position_m().distance(home) < 2.0
                && now > 5.0
            {
                went_home = true;
                break;
            }
        }

        assert!(drew, "the servant reached the front of the queue and drew");
        assert!(windlass_events > 0, "the well's windlass was emitted as a world sound");
        assert!(remembered, "the drawer remembers drawing, so they can be asked about it");
        assert!(drew_at_max, "the draw refilled the servant's thirst");
        assert!(went_home, "the servant walked home again after drawing");
    }

    /// Household vessels go before trade vessels in an ordinary queue, arrival
    /// order preserved within each class (`lore/wells_and_water.md`).
    #[test]
    fn household_vessels_queue_ahead_of_trade_vessels() {
        let mut round = WaterRound::default();
        round.sources.push(WaterSource {
            name: "Test Well".into(),
            draw_point: Vec3::new(0.0, WALK_Y, 0.0),
            draw_sound: "draw_water",
            keeper: Some(ActorId::from_raw("k")),
            queue: Vec::new(),
            serving: None,
            keeper_next_sound: 0.0,
        });
        let waterer = |household: bool| Waterer {
            home: Vec3::ZERO,
            source: 0,
            is_household: household,
            phase: Phase::Idle,
            next_decision: 0.0,
            epoch: 0,
        };
        for (id, household) in [
            ("trade_a", false),
            ("house_a", true),
            ("trade_b", false),
            ("house_b", true),
        ] {
            round.waterers.insert(ActorId::from_raw(id), waterer(household));
            enqueue(&mut round, 0, ActorId::from_raw(id));
        }
        let order: Vec<&str> = round.sources[0].queue.iter().map(ActorId::as_str).collect();
        // Households first (in arrival order), then trades (in arrival order).
        assert_eq!(order, ["house_a", "house_b", "trade_a", "trade_b"]);
    }

    /// Thirst falls by the game clock, and the debug time-scale speeds it up.
    #[test]
    fn thirst_decays_by_the_game_clock() {
        let nav = nav();
        let clock = clock();
        let ford = nav.place("Ford Well").unwrap().node;
        let mut world = base_world();
        world.add_character(person("keeper", nav.node_point(ford), Some("mason")));
        world.add_character(person(
            "servant",
            nav.node_point(nav.adjacency()[ford][0].to),
            Some(HOUSEHOLD_OCCUPATIONS[0]),
        ));

        let mut round = WaterRound::new();
        round.seed(&mut world, &nav, 0.0, &clock);
        let servant = ActorId::from_raw("servant");
        // Top the servant up so the well trip does not refill it during the test.
        world.characters.get_mut(&servant).unwrap().state.needs.thirst = THIRST_MAX;

        // One game hour is 3600 s of game time; at 3600 s/day that is 1/24 day.
        let one_game_hour = 3600.0 / 24.0;
        tick(&mut round, &mut world, &nav, &clock, one_game_hour, &player());
        let expected = THIRST_MAX - 3600.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
        let thirst = world.characters[&servant].needs().thirst;
        assert!(
            (thirst - expected).abs() < 1.0,
            "thirst {thirst} decayed to ~{expected} over one game hour"
        );
    }

    /// No lore-bearing drawers and no keeper means an inert round — the frozen
    /// fixtures' world (no nav, no lore) never grows a queue or a mover.
    #[test]
    fn the_round_is_inert_without_drawers() {
        let nav = nav();
        let clock = clock();
        let mut world = base_world();
        // A lone ambient non-drawer: eligible to keep a well, but nobody draws.
        world.add_character(person("stranger", Vec3::new(89.4, WALK_Y, 36.1), Some("mason")));
        let mut round = WaterRound::new();
        round.seed(&mut world, &nav, 0.0, &clock);
        for step in 1..50 {
            tick(&mut round, &mut world, &nav, &clock, step as f64 * 0.5, &player());
        }
        assert!(
            round.sources().iter().all(|source| source.queue.is_empty()),
            "no drawer, no queue"
        );
        assert!(
            world.characters[&ActorId::from_raw("stranger")].state.movement.is_none(),
            "a keeper never walks"
        );
    }

    /// The deterministic decision hash gives the same city every run.
    #[test]
    fn the_decision_hash_is_stable() {
        let id = ActorId::from_raw("servant");
        assert_eq!(hash01("water_decision", &id, 3), hash01("water_decision", &id, 3));
        assert_ne!(hash01("water_decision", &id, 3), hash01("water_decision", &id, 4));
        // The jitter always lands inside the 1–6 s cadence band.
        for epoch in 0..64 {
            let jitter = decision_jitter(&id, epoch);
            assert!((LADDER_DECISION_MIN_SECONDS..=LADDER_DECISION_MAX_SECONDS).contains(&jitter));
        }
    }
}
