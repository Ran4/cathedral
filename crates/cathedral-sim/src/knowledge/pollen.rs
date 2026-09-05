//! The transport half: the ward's air, the rolls that move a word into a mouth,
//! and (from M2's second half) the instrument that measures how fast
//! (`features/knowledge_and_rumor/02_rumor_pollen.md`).
//!
//! Two things every reader of this file needs to know before writing in it.
//! **The roll is a hash of stable inputs, never a fresh draw** — the engine polls
//! at ~60 Hz and a re-drawn 1-in-20 is a certainty within a frame, which
//! `attention.rs` learned the hard way — and **every deadline is in game-days**,
//! or the `T` key's 60× would change the roll count and the measured cadence
//! would not be the cadence a played run produces.
//!
//! λ, the air's cooling factor per game hour, is
//! [`cooling_lambda`](super::cooling_lambda) in the parent module: `Held::heat`
//! already derives from it, and the sweep must read that one rather than define
//! a second λ beside it.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::OnceLock;

use crate::areas::{Area, AreaMap};
use crate::character::{Character, Control};
use crate::clock::WorldClock;
use crate::ids::{ActorId, AreaKey, FactId, FactKey};
use crate::knowledge::salience::{salience_for, salience_for_listener};
use crate::knowledge::{
    AIR_HALF_LIFE_GAME_HOURS, Fact, FactView, GARBLE_AREA_RADIUS_M, HEAT_GONE_BELOW, HOP_LOSS,
    Held, Holding, PLAYER_CURIOSITY, STIRS_PER_GAME_HOUR, Telling, Topic, VOLUNTEER_HEAT,
    WARD_CELL_M, heat_pct, may_carry, volunteers_with,
};
use crate::lore::PlanningWard;
use crate::math::Vec3;
use crate::world::World;

/// The walkable city's own world box, read off `assets/world/navigation.json`'s
/// `grid` (`x0 -365.0`, `z0 -480.5`, `cell_m 0.25`, `w 2907`, `h 3313`) →
/// 726.75 × 828.25 m.
///
/// **Not the door hull** (x ∈ [-323.6, 329.1], z ∈ [-457.1, 324.9]): a grid sized
/// to the doors leaves real city paving unanswered, and a person standing on it
/// would carry and deposit nothing.
const CITY_MIN_X: f64 = -365.0;
const CITY_MIN_Z: f64 = -480.5;
const CITY_SPAN_X: f64 = 726.75;
const CITY_SPAN_Z: f64 = 828.25;

/// A cell whose centre and corners disagree. Eight wards means ordinals 0..=7, so
/// this can never collide with one.
const AMBIGUOUS: u8 = 0xFF;

/// The word in one ward's air: what is being said there, how loudly, at how few
/// removes, and by whose mouth it last got in.
#[derive(Debug, Clone, PartialEq)]
pub struct Drift {
    pub heat: f32,
    /// The fewest hops any depositing carrier holds it at. A pickup lands at
    /// `hops + 1`.
    pub hops: u8,
    /// The last mouth to deposit at that hop count — the chain link a pickup
    /// records as its `from`, which is what keeps walk-the-chain walkable.
    ///
    /// `Option<ActorId>` is `String`-backed, so **`Drift` is not `Copy` and every
    /// pass whose decision needs `&World` while its write needs `&mut World` is
    /// two-phase**: collect keys, then loop `get_mut`, exactly as `marks::sweep`
    /// does. A single pass will not compile, and the obvious fix — cloning the air
    /// map per poll — is a 20 Hz allocation.
    pub via: Option<ActorId>,
    /// One pickup roll per person per fact per stir — never a fresh draw per poll.
    /// Bumped by the cooling sweep on a fixed half-game-hour grid and by
    /// `Knowledge::stir_up`; **never by a deposit**. Seeded from
    /// [`stage_stir`] at creation, never from 0, so a row evicted and later
    /// re-deposited does not replay the same coins for everyone who was present
    /// the first time.
    pub stir: u32,
}

/// Position → standing ward, exactly, in one array index most of the time.
///
/// An **accelerator, not an approximation**. A cell holds a ward ordinal only
/// where its centre *and all four corners* agree under the exact nearest-mark
/// search, and [`AMBIGUOUS`] otherwise; an ambiguous cell — and any point outside
/// the city box — falls through to that same exact search. So the grid's answer is
/// identical to the exact search *everywhere*, provably, because the fallback
/// **is** the exact search. That is what lets `crowd.rs` be routed through it
/// without moving anybody's house.
#[derive(Debug, Clone, PartialEq)]
pub struct WardGrid {
    cells: Vec<u8>,
    cols: usize,
    rows: usize,
    /// The 321 ward-labelled baked doors, in `homes::ward_marks()`'s sorted
    /// order. Held here so the exact search never re-parses `homes.json`.
    marks: Vec<([f64; 2], PlanningWard)>,
}

/// The grid, baked once per process: 91 × 104 = 9,464 bytes plus the marks.
pub fn ward_grid() -> &'static WardGrid {
    static GRID: OnceLock<WardGrid> = OnceLock::new();
    GRID.get_or_init(WardGrid::bake)
}

impl WardGrid {
    pub fn bake() -> Self {
        let marks = crate::crowd::ward_map();
        let cols = (CITY_SPAN_X / WARD_CELL_M).ceil() as usize;
        let rows = (CITY_SPAN_Z / WARD_CELL_M).ceil() as usize;
        let mut cells = vec![AMBIGUOUS; cols * rows];
        if marks.is_empty() {
            return Self {
                cells,
                cols,
                rows,
                marks,
            };
        }
        for cx in 0..cols {
            for cz in 0..rows {
                let x = CITY_MIN_X + cx as f64 * WARD_CELL_M;
                let z = CITY_MIN_Z + cz as f64 * WARD_CELL_M;
                let centre = exact_over(&marks, x + WARD_CELL_M / 2.0, z + WARD_CELL_M / 2.0);
                let agreed = [
                    (0.0, 0.0),
                    (WARD_CELL_M, 0.0),
                    (0.0, WARD_CELL_M),
                    (WARD_CELL_M, WARD_CELL_M),
                ]
                .iter()
                .all(|(dx, dz)| exact_over(&marks, x + dx, z + dz) == centre);
                if agreed && let Some(ward) = centre {
                    cells[cz * cols + cx] = ordinal(ward);
                }
            }
        }
        Self {
            cells,
            cols,
            rows,
            marks,
        }
    }

    /// **The** definition of where a person is standing.
    pub fn at(&self, point: Vec3) -> Option<PlanningWard> {
        let cx = ((point.x - CITY_MIN_X) / WARD_CELL_M).floor();
        let cz = ((point.z - CITY_MIN_Z) / WARD_CELL_M).floor();
        if cx >= 0.0 && cz >= 0.0 {
            let (cx, cz) = (cx as usize, cz as usize);
            if cx < self.cols && cz < self.rows {
                let cell = self.cells[cz * self.cols + cx];
                if cell != AMBIGUOUS {
                    return Some(PlanningWard::ALL[usize::from(cell)]);
                }
            }
        }
        self.exact_at(point)
    }

    /// The exact nearest-mark search — `crowd::nearest_ward`'s body, moved here so
    /// there is one copy of it. The tie-break is strictly `<`, so the answer
    /// depends on `homes::ward_marks()`' sort order and that sort is therefore
    /// load-bearing.
    pub fn exact_at(&self, point: Vec3) -> Option<PlanningWard> {
        exact_over(&self.marks, point.x, point.z)
    }

    /// Whether this city has a ward map at all — every hand-built test nav, where
    /// nobody has a ward and `crowd::doorsteps` houses nobody.
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn cells(&self) -> usize {
        self.cells.len()
    }

    pub fn ambiguous_cells(&self) -> usize {
        self.cells.iter().filter(|cell| **cell == AMBIGUOUS).count()
    }
}

fn exact_over(marks: &[([f64; 2], PlanningWard)], x: f64, z: f64) -> Option<PlanningWard> {
    let mut best: Option<(f64, PlanningWard)> = None;
    for ([mx, mz], ward) in marks {
        let (dx, dz) = (mx - x, mz - z);
        let distance = dx * dx + dz * dz;
        if best.is_none_or(|(closest, _)| distance < closest) {
            best = Some((distance, *ward));
        }
    }
    best.map(|(_, ward)| ward)
}

/// Declaration order, which is [`PlanningWard::ALL`]'s order.
fn ordinal(ward: PlanningWard) -> u8 {
    PlanningWard::ALL
        .iter()
        .position(|other| *other == ward)
        .expect("PlanningWard::ALL is exhaustive") as u8
}

/// Which named areas a garbled place may become: `AreaMap::nearest_areas` minus
/// the area itself, precomputed once from the live map.
///
/// Built from the [`AreaMap`] and **never bolted onto the embedded
/// `FactCatalog`**: the catalog is an `include_str!` asset with one lifecycle,
/// and a host-supplied, world-dependent index on it would give one type two.
/// Empty in every world with an empty map (the goldens, every hermetic test), so
/// a place garble is a no-op there — stated, not discovered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AreaAdjacency {
    /// Indexed by [`AreaKey`], which is the index into `AreaMap::areas` and
    /// therefore JSON order — stable across runs and goldens.
    neighbours: Vec<Vec<AreaKey>>,
}

impl AreaAdjacency {
    /// Five neighbours at most, inside [`GARBLE_AREA_RADIUS_M`], measured from the
    /// arithmetic mean of the area's boxes' centres. Six are asked for and the
    /// area itself dropped, because `nearest_areas` returns the query's own area
    /// at distance 0.
    pub fn build(map: &AreaMap) -> Self {
        let mut neighbours = Vec::with_capacity(map.areas.len());
        for (index, area) in map.areas.iter().enumerate() {
            let centre = centroid(area);
            let mut near: Vec<AreaKey> = map
                .nearest_areas(centre, GARBLE_AREA_RADIUS_M, 6)
                .into_iter()
                .filter(|found| found.area.id != area.id)
                .filter_map(|found| map.key_of_id(&found.area.id))
                .collect();
            near.truncate(5);
            debug_assert!(u16::try_from(index).is_ok());
            neighbours.push(near);
        }
        Self { neighbours }
    }

    pub fn neighbours(&self, area: AreaKey) -> &[AreaKey] {
        self.neighbours
            .get(usize::from(area.0))
            .map_or(&[], Vec::as_slice)
    }
}

fn centroid(area: &Area) -> Vec3 {
    let mut sum = Vec3::new(0.0, 0.0, 0.0);
    for bounds in &area.boxes {
        sum += (bounds.min_m + bounds.max_m) * 0.5;
    }
    sum * (1.0 / area.boxes.len().max(1) as f64)
}

/// The one place the roll's probability is computed.
///
/// [`may_carry`] first: the subject's chance is 0.0 whatever the table says, under
/// every measurement lever. The player is rolled at [`PLAYER_CURIOSITY`] and
/// **never** through `attention::curiosity_of`, which returns
/// `CURIOSITY_WITHOUT_LORE` (1.0) for a body with no lore sheet — and
/// `assets/world/seed.json` gives the player `{"id":"player","name":"Player",
/// "control":"player"}` and no lore. Rolled that way they would hold every fact in
/// the ward's air on their first poll, the journal would be a firehose, and "beat
/// your own story to the ward" would be dead.
///
/// `pub` for one reader outside the crate: the cadence harness folds it over
/// every authored body × every topic to show the clamp never binds
/// (`the_pickup_chance_never_clamps`). Gameplay reaches the roll only through
/// [`poll_person`], which uses the resolved-listener form below.
pub fn pickup_chance(world: &World, fact: &Fact, listener: &ActorId, heat: f32) -> f64 {
    pickup_chance_of(world, fact, &Listener::resolve(world, listener), heat)
}

/// [`pickup_chance`] for a listener already resolved — the pickup arm's shape,
/// so the innermost roll pays no `characters.get`.
pub(crate) fn pickup_chance_of(
    world: &World,
    fact: &Fact,
    listener: &Listener<'_>,
    heat: f32,
) -> f64 {
    if !may_carry(fact, listener.id) {
        return 0.0;
    }
    (listener.curiosity * f64::from(heat) * salience_for_listener(world, fact, listener))
        .clamp(0.0, 1.0)
}

/// One listener, resolved **once per poll**: everything a roll needs to know
/// about the person rolling — who, how curious, what trade, whether they are the
/// player — read off `characters` one time, instead of once per air row per
/// factor. `characters.get` is a `BTreeMap<String, _>` probe, and at 20,000
/// bodies the roll was paying four to six of them per (person, row) pair.
pub(crate) struct Listener<'a> {
    pub(crate) id: &'a ActorId,
    /// The curiosity **the roll spends**: [`PLAYER_CURIOSITY`] for the player and
    /// `curiosity_of`'s answer for everybody else — including its
    /// `CURIOSITY_WITHOUT_LORE` = 1.0 for a lore-less NPC, which is exactly what
    /// the player must never be rolled at.
    pub(crate) curiosity: f64,
    pub(crate) occupation: Option<&'a str>,
    pub(crate) is_player: bool,
}

impl<'a> Listener<'a> {
    pub(crate) fn resolve(world: &'a World, id: &'a ActorId) -> Self {
        let body = world.characters.get(id);
        let is_player = body.is_some_and(|body| body.control() == Control::Player);
        let lore = body.and_then(Character::lore);
        let curiosity = if is_player {
            PLAYER_CURIOSITY
        } else {
            lore.map_or(
                crate::attention::CURIOSITY_WITHOUT_LORE,
                crate::attention::curiosity_from_lore,
            )
        };
        Self {
            id,
            curiosity,
            occupation: lore.and_then(|lore| lore.occupation_id.as_deref()),
            is_player,
        }
    }
}

/// The curiosity **the roll actually uses** — one function so the census's
/// per-ward mean is the same number the roll spends, and not
/// `curiosity_of`'s 1.0 lore-less fallback for the one body that has no sheet.
fn curiosity_for(world: &World, listener: &ActorId) -> f64 {
    Listener::resolve(world, listener).curiosity
}

/// What this actor holds of one fact, by the same seeded-before-stored order
/// [`crate::knowledge::holds_key`] answers in, off rows already fetched — no map
/// probe and no `Held` clone per air row.
fn held_hops(rows: &[Holding], fact: &Fact, key: FactKey, actor: &ActorId) -> Option<u8> {
    if fact.seeded.contains(actor) {
        return Some(0);
    }
    rows.binary_search_by(|row| row.key.cmp(&key))
        .ok()
        .map(|at| rows[at].hops)
}

/// The one roll, and it is a hash of stable inputs and never a fresh draw —
/// `notices::carries`' idiom with the two guards `custody::struggle_roll` has and
/// `carries` lacks.
///
/// Hashed over `stir` and **not** over the clock: that is what stops a 1-in-20
/// chance becoming a certainty within a few seconds of polling.
///
/// Not `world::hash01`, which takes only `(salt, id, epoch)`: the
/// **`fact.sequence` term is load-bearing**, not for the probability — it
/// contributes nothing to it — but to decorrelate the rolls, so the same mouths do
/// not pick up everything in the air at once.
pub fn picks_up(fact: &Fact, actor: &ActorId, stir: u32, chance: f64) -> bool {
    if chance.is_nan() || chance <= 0.0 {
        return false;
    }
    if chance >= 1.0 {
        return true;
    }
    let mut hasher = DefaultHasher::new();
    "pollen_pickup".hash(&mut hasher);
    fact.sequence.hash(&mut hasher);
    actor.hash(&mut hasher);
    stir.hash(&mut hasher);
    ((hasher.finish() >> 11) as f64 / (1u64 << 53) as f64) < chance
}

/// The stir grid's ordinal at an instant — the same half-game-hour grid the sweep
/// bumps `stir` on, so a row created now starts on the current coin and not on
/// coin 0.
pub(crate) fn stage_stir(game_days: f64) -> u32 {
    (game_days.max(0.0) * 24.0 * STIRS_PER_GAME_HOUR) as u32
}

/// One person's turn at the air: deposit what they are warm enough to be saying,
/// then roll once for everything in this ward they do not already hold closer.
///
/// One ward lookup and at most [`AIR_PER_WARD_MAX`](super::AIR_PER_WARD_MAX)
/// rolls. Both halves are collected under `&World` and applied under
/// `&mut World`, so a fact picked up on this poll is not deposited on the same
/// poll, and `Drift`'s non-`Copy` `via` costs no clone of the map.
///
/// Writes the store and nothing else: **a pickup is not a percept**.
/// `Novelty::admits_idle` returns `true` unconditionally on a non-empty inbox
/// (`attention.rs`), so one `notify_percept` per pickup would turn the pollen
/// field into a city-wide prompt firehose at `extra_ambient_npcs: 1000`. There is
/// no `notify_percept` anywhere in this function and there must never be one.
pub fn poll_person(world: &mut World, actor: &ActorId, game_days: f64) {
    if !world.knowledge_enabled || world.knowledge.is_empty() {
        return;
    }
    let Some(position) = world.characters.get(actor).map(Character::position_m) else {
        return;
    };
    let Some(ward) = world.ward_at(position) else {
        return;
    };
    // The listener and their rows, resolved once: every probe below is a slice
    // search or a `FactKey` lookup, never a `characters.get` or a `holdings.get`
    // per row.
    let listener = Listener::resolve(world, actor);
    let rows = world.knowledge.rows_of(actor);

    // ── phase 1a: what is this mouth warm enough to be saying?
    //
    // A walk over `live` to find the seeded rows, so this is O(facts) per poll —
    // costed and decided in `02_numbers.md` §6 (M2 ships without a reverse
    // index; `seeded_by_actor` is the named fix if the saturated cost guard's
    // ratio exceeds 15%). The gate is `volunteers_with`, the one expression.
    let mut deposits: Vec<(FactKey, u8, f32)> = Vec::new();
    for (key, fact) in world.knowledge.facts() {
        if !fact.decays || !may_carry(fact, actor) {
            continue;
        }
        let (hops, heat) = if fact.seeded.contains(actor) {
            (0, Held::seeded(fact).heat(Some(game_days)))
        } else {
            match rows.binary_search_by(|row| row.key.cmp(&key)) {
                Ok(at) => (rows[at].hops, rows[at].heat(Some(game_days))),
                Err(_) => continue,
            }
        };
        let salience = salience_for_listener(world, fact, &listener);
        if !volunteers_with(fact, actor, heat, salience) {
            continue;
        }
        deposits.push((key, hops, heat));
    }

    // ── phase 1b: one roll per fact in this ward's air
    let mut tellings: Vec<(FactKey, Telling)> = Vec::new();
    for (key, drift) in world.knowledge.ward_air(ward) {
        let Some(fact) = world.knowledge.fact(key) else {
            continue;
        };
        // The subject never rolls — stated here as well as inside `pickup_chance`,
        // so a reader does not have to know it returns 0.0.
        if !may_carry(fact, actor) {
            continue;
        }
        let hops = drift.hops.saturating_add(1);
        if held_hops(rows, fact, key, actor).is_some_and(|have| have <= hops) {
            continue;
        }
        let chance = pickup_chance_of(world, fact, &listener, drift.heat);
        if !picks_up(fact, actor, drift.stir, chance) {
            continue;
        }
        tellings.push((
            key,
            Telling {
                hops,
                from: drift.via.clone(),
                heat: drift.heat * HOP_LOSS,
                // M3 replaces this with `garble::view_for(world, fact, actor, hops)`.
                view: FactView::default(),
            },
        ));
    }

    // ── phase 2: the writes
    for (key, hops, heat) in deposits {
        world
            .knowledge
            .deposit(ward, key, hops, heat, actor, game_days);
    }
    for (key, telling) in tellings {
        crate::knowledge::learn(world, actor, key, telling, Some(game_days));
    }
}

/// The player's own seat at the air. They are not in `round.people`
/// (`Round::seed` filters on `control().is_llm()`) and `notify_percept` no-ops for
/// them, so this is called from `Engine::poll` on its own game-time deadline.
///
/// Identical to [`poll_person`] but for the curiosity, which [`pickup_chance`]
/// already branches on — so this is one call, not a second mechanism. M4 adds the
/// receipt (`player_learned`, the journal); M2 makes them a carrier.
pub fn poll_player(world: &mut World, player_id: &ActorId, game_days: f64) {
    poll_person(world, player_id, game_days);
}

/// Cool the air, bump `stir`, evict what has gone, and hold each ward to
/// [`AIR_PER_WARD_MAX`](super::AIR_PER_WARD_MAX).
///
/// Self-gating on the stir grid via `Knowledge::take_stir_beat`, which is
/// `marks::sweep`'s shape *including* its keep-the-clock-current early-out, so the
/// first fact minted into a long-running world is not charged for the whole run.
///
/// Returns whether anything moved — for tests and for `--trace-pollen` only.
/// **It must never reach `touch_public_state`**: facts are prompt state, like
/// notices, and the snapshot's 160 KiB bound has little headroom left.
///
/// Cooling is the one pass here that is *not* two-phase, and the reason is worth
/// stating so nobody "fixes" it: a pass needs two phases when its decision needs
/// `&World` while its write needs `&mut World`, which is [`poll_person`]'s pickup
/// arm and will be M3's `hop_on_stage`. Cooling needs nothing from the world but a
/// factor, so it holds one `&mut` to the map and walks it once — one
/// `Arc::make_mut` instead of N. What must not be done, in either shape, is clone
/// the air map per pass: that is a 20 Hz allocation.
pub fn sweep(world: &mut World, game_days: f64) -> bool {
    if !world.knowledge_enabled {
        return false;
    }
    let previous = world.knowledge.last_sweep_game_days();
    if !world.knowledge.take_stir_beat(game_days) {
        return false;
    }
    // On the very first beat nothing has aged: the air was just deposited.
    //
    // Cooling charges the exact elapsed time, and the coin advances by the number
    // of grid edges that elapsed — one per half game hour — so both are functions
    // of game time and not of how often the host polled. A stall longer than one
    // stir window (75 s at the shipped clock, 1.25 s under the `T` key's 60×)
    // would otherwise mint one coin where several were due and quietly lower the
    // roll rate below `STIRS_PER_GAME_HOUR`.
    let beats = STIRS_PER_GAME_HOUR * 24.0;
    let (elapsed_hours, edges) = if previous.is_finite() {
        (
            ((game_days - previous) * 24.0).max(0.0),
            ((game_days * beats).floor() - (previous * beats).floor()).max(1.0) as u32,
        )
    } else {
        (0.0, 1)
    };
    let factor = super::cooling_lambda().powf(elapsed_hours) as f32;

    let mut changed = false;
    let mut gone: Vec<(PlanningWard, FactKey)> = Vec::new();
    {
        let air = world.knowledge.air_mut();
        for (key, drift) in air.iter_mut() {
            let before = heat_pct(drift.heat);
            drift.heat *= factor;
            drift.stir = drift.stir.wrapping_add(edges);
            if heat_pct(drift.heat) != before {
                changed = true;
            }
            // `HEAT_GONE_BELOW` is **0.01 and never 0.0**: exponential decay
            // underflows to exactly zero and `0.0 < 0.0` is false forever, which
            // makes the row immortal and holds a cap slot nothing can reclaim —
            // the bug `MarkCatalog::from_json` refuses in as many words.
            if drift.heat < HEAT_GONE_BELOW {
                gone.push(*key);
            }
        }
        for key in &gone {
            air.remove(key);
        }
    }
    changed |= !gone.is_empty();
    // The per-ward cap, coldest out — the same total order the holdings cap evicts
    // by, so there is one eviction rule in the feature and it is reproducible.
    changed |= world.knowledge.trim_air_to_cap();
    changed
}

/// One `Drift` row into one ward's air at full heat, for the measurement levers
/// and nothing else: `--pollen-saturate`'s nine-rows-per-ward cost guard
/// (`02_numbers.md` §6) and M4's `seed-fact … -> <ward>`.
///
/// `#[doc(hidden)]`, like [`Knowledge::rewind_sweep_clock`](super::Knowledge::rewind_sweep_clock):
/// it is reachable because a host binary needs it, not because it is part of the
/// model. Respects [`AIR_PER_WARD_MAX`](super::AIR_PER_WARD_MAX), because
/// `seed_air` trims.
#[doc(hidden)]
pub fn debug_seed_air(world: &mut World, ward: PlanningWard, key: FactKey, game_days: Option<f64>) {
    world
        .knowledge
        .seed_air(ward, key, 1.0, game_days.unwrap_or(0.0));
}

// ---------------------------------------------------------------------------
// The instrument (M2 step 17)
// ---------------------------------------------------------------------------

/// What the wave is actually doing, per fact — the cadence band's own instrument.
///
/// Printed by `--trace-pollen`; the assertions in
/// `crates/cathedral-backends/tests/pollen_cadence.rs` read these fields and
/// nothing else, so a change to the print cannot quietly change a test.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PollenCensus {
    /// The instant this is a census *of*, so a printed sample says which bell it
    /// belongs to without the caller having to say it beside every line.
    pub game_days: f64,
    pub facts: usize,
    pub holdings: usize,
    pub air_entries: usize,
    pub store_bytes: usize,
    /// The poll-gap invariant's **bound** on the roll rate, not a counter: one
    /// roll per person per fact per stir at most, so
    /// [`STIRS_PER_GAME_HOUR`] × Σ over present people |air(their ward)|. The
    /// real count is lower — `poll_person` never rolls a row its listener already
    /// holds at `hops + 1` or closer, nor the subject on their own fact — and no
    /// counter exists because the invariant is what is asserted
    /// (`the_poll_gap_cannot_skip_a_stir`). A load figure, printed as
    /// `rolls/gh(bound)`.
    pub rolls_per_game_hour: f64,
    /// Measured X, city mean. Filled by the caller from a [`TallySnapshot`];
    /// `0.0` from a single instant, because no instant can answer it.
    pub crossings_per_person_per_game_day: f64,
    /// The undebounced flip count beside it, for the print only.
    pub crossings_per_person_per_game_day_raw: f64,
    /// Measured X **per ward**: exits ÷ person-game-hours in it × 24. The mint
    /// ward's row is what every expectation divides by (D54).
    pub ward_exit_rate_per_game_day: BTreeMap<PlanningWard, f64>,
    /// Per-ward, never a city mean: the standing wards are 7:1 lopsided
    /// (BellAndSluice 192, Cinder 26 on the shipped cast), so a mean hides both
    /// ends of every figure below.
    pub ward_population: BTreeMap<PlanningWard, usize>,
    /// The mean of the curiosity **the roll spends** — so the player's row is
    /// [`PLAYER_CURIOSITY`] and not `curiosity_of`'s lore-less 1.0.
    pub ward_mean_curiosity: BTreeMap<PlanningWard, f64>,
    /// One row per live fact, ordered by [`Topic::ALL`] then [`FactKey`] — stable,
    /// so two runs diff cleanly.
    pub topics: Vec<TopicRow>,
    /// The dense handle of each row of [`Self::topics`], positionally.
    ///
    /// The join [`Self::fill`] needs and nothing else: a [`TallySnapshot`] is keyed
    /// by [`FactKey`] while a row names its [`FactId`], and a handle must never be
    /// rendered (D10) — so it lives here rather than on the row.
    pub keys: Vec<FactKey>,
}

/// One fact's whole story at one instant: the air's half (where it is being said)
/// and the mouths' half (who could say it).
#[derive(Debug, Clone, PartialEq)]
pub struct TopicRow {
    pub topic: Topic,
    pub fact: FactId,
    /// The ward the fact was minted in, read back off the air: the row
    /// [`mint::install_fact`](super::mint::install_fact) seeded at `hops: 0` with
    /// **no `via`**, which is by construction `world.ward_at(<the mint point>)`.
    ///
    /// A deposit always writes `via: Some`. Two writers leave `via: None`:
    /// `seed_air` (the mint) and `Knowledge::stir_up` creating a row in a ward
    /// whose air lacked it (a re-heat — the standing-fact test, M4's `reheat`).
    /// So this reading is exact until a fact is re-heated in a ward other than
    /// its own, or under `--pollen-saturate`; then two such rows exist and the
    /// lowest ward ordinal wins, deterministically. [`CrossingTally`] caches the
    /// answer at its **first** sample — when only the seed row can exist — and
    /// [`PollenCensus::fill`] divides by that cached ward, so `expected_crossings`
    /// is never moved by a later re-heat or by the seed row cooling out of the
    /// air at 79.7 gh. `None` once no such row is in the air.
    pub mint_ward: Option<PlanningWard>,
    /// **The number of wards whose air holds a row for this fact** — deposits,
    /// never holders' wards (D54). What every realised band backstop reads.
    pub wards_reached: u8,
    pub carriers: usize,
    /// How many of those carriers are warm enough to repeat it — the deposit
    /// gate's own population, which is what the slow end turns on.
    pub volunteering: usize,
    /// `warm_mint_ward_game_hours × ward_exit_rate(mint_ward) / 24` — the model's
    /// expected crossings out of the mint ward so far. Filled from a snapshot.
    pub expected_crossings: f64,
    /// Realised exits of the mint ward by anyone holding it, warm or cold — a
    /// printed diagnostic, never asserted (D54).
    pub holder_exits: f64,
    pub holder_exits_same_trade: f64,
    /// Exits of **any** ward by a same-trade holder still warm on it — the
    /// same-trade ear's reported figure (`TallySnapshot::warm_same_trade_exits`).
    pub warm_same_trade_exits: f64,
    /// The three holder statistics, beside `carriers` and `volunteering`:
    /// the mean removes of everyone who holds it, and the warmest mouth in the
    /// city — which is the one that decides whether it can still travel at all.
    pub mean_hops: f64,
    pub max_heat: f32,
    pub age_game_hours: f64,
    pub carriers_by_ward: BTreeMap<PlanningWard, usize>,
}

/// The one thing no instant can answer: how often people change wards, and how
/// often a **holder** leaves the ward a fact was minted in.
///
/// Held by whoever is sampling — the headless tracer, the cadence test — and
/// never by [`World`], so a production run pays nothing for it. The per-fact half
/// runs only for the keys it was built with.
///
/// **A settled-ward count, debounced two ways**, because a raw boundary count is
/// a function of the sample rate and not of the city: a person milling on their
/// idle leash beside a ward line flips wards at every sample, and the raw count
/// read 3.79 / 8.21 / 12.21 crossings per person per game day at 12 / 48 / 192
/// samples a day against the leg-derived 2.4 of `02_numbers.md` §1. A ward
/// change is believed only when (1) the body has displaced at least
/// [`SETTLED_DISPLACEMENT_M`] from where it was last seen settled — a mill never
/// can, a walker always does — **and** (2) the new ward is still there at the
/// next sample. A transit through a third ward inside one gap therefore counts
/// once (the leg count the plan's X is) and a round trip inside one gap counts
/// nothing: the tally can only **undercount**, never grow with the cadence.
/// Sample at least twice a game hour; a walker covers ~150 m per half game hour
/// at the shipped clock.
pub struct CrossingTally {
    watching: Vec<FactKey>,
    /// Resolved from the air on the first sample that can answer it, and then
    /// **never re-read**: the expectation must not change its divisor because a
    /// row cooled out of the air late in a run. Carried on every
    /// [`TallySnapshot`], so [`PollenCensus::fill`] divides by this ward and not
    /// by whatever the census reads off the air at that instant.
    mint_ward: BTreeMap<FactKey, PlanningWard>,
    standing: BTreeMap<ActorId, Stand>,
    changes: f64,
    /// Every sample-to-sample ward flip, undebounced — printed beside the
    /// believed count so the debounce's effect is legible, never used by an
    /// expectation.
    raw_changes: f64,
    bodies: usize,
    exits: BTreeMap<PlanningWard, f64>,
    person_game_hours: BTreeMap<PlanningWard, f64>,
    warm_mint_ward_game_hours: BTreeMap<FactKey, f64>,
    holder_exits: BTreeMap<FactKey, f64>,
    holder_exits_same_trade: BTreeMap<FactKey, f64>,
    warm_same_trade_exits: BTreeMap<FactKey, f64>,
    first_game_days: Option<f64>,
    last_game_days: f64,
}

/// How far a body must have moved from where it was last seen **settled** before
/// a ward change is believed. Over twice the widest authored idle leash
/// (`night_watch`, 24 m in `rounds.json`; the cast's default is 10 m): a mill is a
/// fresh point inside a disc no wider than 2 × leash at every sample and can never
/// displace this far, while a walker at 2.1 m/s covers it in 24 s. Measured from
/// the last *settled* sample and not the previous one, so the threshold does not
/// depend on the sample gap. The generated crowd's leash reaches 40 m
/// (`crowd_leash_m`), so at `--extra-ambient` a crowd body straddling a ward line
/// can leak through — one more reason the band is asserted at 0.
const SETTLED_DISPLACEMENT_M: f64 = 50.0;

/// Where one body stands, as the tally believes it.
struct Stand {
    settled: PlanningWard,
    /// The ward at the previous sample, for the raw flip count.
    last: PlanningWard,
    /// The position at the last sample that found them in `settled`.
    seen_settled_at: Vec3,
    /// A ward they were seen in once, far enough from `seen_settled_at` to be a
    /// walk and not a mill, awaiting the next sample's confirmation.
    pending: Option<PlanningWard>,
}

/// Every accumulator at one instant, taken **per sample** and stored beside that
/// sample's census — so a test reads the office boundary and the game day off one
/// run, and takes `X` from the final snapshot only, never from an office sample
/// (the Dayspring tide annualised is ~2× the daily figure).
#[derive(Debug, Clone, PartialEq)]
pub struct TallySnapshot {
    pub game_days: f64,
    pub span_game_days: f64,
    /// City mean: settled-ward changes ÷ bodies ÷ elapsed game days (see
    /// [`CrossingTally`] for the debounce).
    pub per_person_per_game_day: f64,
    /// The same figure undebounced — every sample-to-sample flip — so the
    /// debounce's effect is legible in a print. Never an expectation's input.
    pub per_person_per_game_day_raw: f64,
    /// Per ward: exits ÷ person-game-hours spent settled in it × 24 — **the**
    /// `X_W` every expectation uses (D54).
    pub ward_exit_rate_per_game_day: BTreeMap<PlanningWard, f64>,
    /// The mint ward the tally resolved for each watched key — the divisor
    /// [`PollenCensus::fill`] uses, so a row cooling out of the air or a re-heat
    /// into another ward cannot move it.
    pub mint_ward: BTreeMap<FactKey, PlanningWard>,
    /// Per watched key: Σ over samples, and over every holder settled in its mint
    /// ward, of the part of the sample gap they were **warm** for — the closed
    /// form's own integrand (`K × t_warm` for the seeded mouths).
    ///
    /// Integrated in closed form per holder rather than counted at the gap's end:
    /// a holder's warm life from now is `τ · log₂(heat(now) × salience /
    /// VOLUNTEER_HEAT)` game hours (negative once cold), so the warm span inside
    /// `[previous sample, now]` is `[max(previous, learned_on), min(now, cold_at)]`
    /// clamped to the gap. A right-endpoint count read **zero** for any warm life
    /// shorter than the gap — the slow end's 0.145 gh against a 0.5 gh gap — and so
    /// asserted nothing; this reads `4 × 0.145` at any cadence.
    pub warm_mint_ward_game_hours: BTreeMap<FactKey, f64>,
    pub holder_exits: BTreeMap<FactKey, f64>,
    pub holder_exits_same_trade: BTreeMap<FactKey, f64>,
    /// Per watched key: exits of **any** settled ward by a holder of the
    /// subject's own trade who was still warm on it at the crossing — the
    /// same-trade ear's own figure (`02_numbers.md` §4: ≈ 2.1 a game day per
    /// warm witness), which the mint-ward split above cannot see when the
    /// trade's mouths stand in other wards. Reported, never asserted.
    pub warm_same_trade_exits: BTreeMap<FactKey, f64>,
}

impl CrossingTally {
    pub fn new(watching: &[FactKey]) -> Self {
        Self {
            watching: watching.to_vec(),
            mint_ward: BTreeMap::new(),
            standing: BTreeMap::new(),
            changes: 0.0,
            raw_changes: 0.0,
            bodies: 0,
            exits: BTreeMap::new(),
            person_game_hours: BTreeMap::new(),
            warm_mint_ward_game_hours: BTreeMap::new(),
            holder_exits: BTreeMap::new(),
            holder_exits_same_trade: BTreeMap::new(),
            warm_same_trade_exits: BTreeMap::new(),
            first_game_days: None,
            last_game_days: 0.0,
        }
    }

    /// One pass over the present cast: where everybody is standing now against
    /// where the tally last believed them settled.
    pub fn sample(&mut self, world: &World, game_days: f64) {
        let gap_game_hours = match self.first_game_days {
            None => 0.0,
            Some(_) => ((game_days - self.last_game_days) * 24.0).max(0.0),
        };
        let previous_game_days = self.last_game_days;
        self.first_game_days.get_or_insert(game_days);
        self.last_game_days = game_days;

        for key in &self.watching {
            if self.mint_ward.contains_key(key) {
                continue;
            }
            if let Some(ward) = minted_in(world, *key) {
                self.mint_ward.insert(*key, ward);
            }
        }

        let Self {
            watching,
            mint_ward,
            standing,
            changes,
            raw_changes,
            bodies,
            exits,
            person_game_hours,
            warm_mint_ward_game_hours,
            holder_exits,
            holder_exits_same_trade,
            warm_same_trade_exits,
            ..
        } = self;

        let mut present = 0usize;
        for (id, character) in &world.characters {
            if !world.is_present(id) {
                continue;
            }
            let position = character.position_m();
            let Some(ward) = world.ward_at(position) else {
                continue;
            };
            present += 1;
            let settled = if let Some(stand) = standing.get_mut(id) {
                if stand.last != ward {
                    *raw_changes += 1.0;
                }
                stand.last = ward;
                let mut left: Option<PlanningWard> = None;
                match stand.pending {
                    // Still there a sample later, and it was a walk: believed.
                    Some(pending) if pending == ward => {
                        left = Some(stand.settled);
                        stand.settled = ward;
                        stand.pending = None;
                        stand.seen_settled_at = position;
                    }
                    // Back, or somewhere else again: a mill, or a leg still going.
                    Some(_) => stand.pending = None,
                    None => {}
                }
                if left.is_none() {
                    if ward == stand.settled {
                        stand.seen_settled_at = position;
                    } else if position.distance(stand.seen_settled_at) >= SETTLED_DISPLACEMENT_M {
                        stand.pending = Some(ward);
                    }
                }
                if let Some(previous) = left {
                    *changes += 1.0;
                    *exits.entry(previous).or_insert(0.0) += 1.0;
                    for key in watching.iter() {
                        if mint_ward.get(key) != Some(&previous) {
                            continue;
                        }
                        if crate::knowledge::holds_key(world, id, *key).is_none() {
                            continue;
                        }
                        *holder_exits.entry(*key).or_insert(0.0) += 1.0;
                        let ear = world
                            .knowledge
                            .fact(*key)
                            .and_then(|fact| fact.craft_ear.as_deref());
                        let trade = character
                            .lore()
                            .and_then(|lore| lore.occupation_id.as_deref());
                        if ear.is_some() && ear == trade {
                            *holder_exits_same_trade.entry(*key).or_insert(0.0) += 1.0;
                        }
                    }
                    // The same-trade ear's own figure: a crossing out of *any*
                    // ward by a holder of the subject's trade while still warm.
                    for key in watching.iter() {
                        let Some(fact) = world.knowledge.fact(*key) else {
                            continue;
                        };
                        let Some(ear) = fact.craft_ear.as_deref() else {
                            continue;
                        };
                        let trade = character
                            .lore()
                            .and_then(|lore| lore.occupation_id.as_deref());
                        if trade != Some(ear) {
                            continue;
                        }
                        let Some(held) = crate::knowledge::holds_key(world, id, *key) else {
                            continue;
                        };
                        if crate::knowledge::volunteers(world, fact, id, &held, Some(game_days)) {
                            *warm_same_trade_exits.entry(*key).or_insert(0.0) += 1.0;
                        }
                    }
                }
                stand.settled
            } else {
                standing.insert(
                    id.clone(),
                    Stand {
                        settled: ward,
                        last: ward,
                        seen_settled_at: position,
                        pending: None,
                    },
                );
                ward
            };
            *person_game_hours.entry(settled).or_insert(0.0) += gap_game_hours;
        }
        *bodies = present;

        // The warm integrand, in closed form per holder (see
        // `TallySnapshot::warm_mint_ward_game_hours`). Only a decaying fact has a
        // warm life at all (D52).
        if gap_game_hours > 0.0 {
            for key in watching.iter() {
                let Some(mint) = mint_ward.get(key).copied() else {
                    continue;
                };
                let Some(fact) = world.knowledge.fact(*key) else {
                    continue;
                };
                if !fact.decays {
                    continue;
                }
                let mut warm_hours = 0.0_f64;
                for (id, stand) in standing.iter() {
                    if stand.settled != mint || !world.is_present(id) || !may_carry(fact, id) {
                        continue;
                    }
                    let Some(held) = crate::knowledge::holds_key(world, id, *key) else {
                        continue;
                    };
                    let salience = salience_for(world, fact, id);
                    let heat = f64::from(held.heat(Some(game_days)));
                    if !(heat > 0.0 && salience > 0.0) {
                        continue;
                    }
                    let remaining = AIR_HALF_LIFE_GAME_HOURS
                        * (heat * salience / f64::from(VOLUNTEER_HEAT)).log2();
                    let cold_at = game_days + remaining / 24.0;
                    let from = held
                        .learned_on
                        .map_or(previous_game_days, |on| on.max(previous_game_days));
                    let to = cold_at.min(game_days);
                    warm_hours += ((to - from) * 24.0).clamp(0.0, gap_game_hours);
                }
                *warm_mint_ward_game_hours.entry(*key).or_insert(0.0) += warm_hours;
            }
        }
    }

    pub fn snapshot(&self) -> TallySnapshot {
        let span =
            (self.last_game_days - self.first_game_days.unwrap_or(self.last_game_days)).max(0.0);
        let rate = |changes: f64| match (self.bodies, span) {
            (0, _) => 0.0,
            (_, span) if span <= 0.0 => 0.0,
            (bodies, span) => changes / bodies as f64 / span,
        };
        let per_person = rate(self.changes);
        let per_person_raw = rate(self.raw_changes);
        let ward_exit_rate_per_game_day = PlanningWard::ALL
            .iter()
            .filter_map(|ward| {
                let hours = *self.person_game_hours.get(ward)?;
                if hours <= 0.0 {
                    return None;
                }
                let exits = self.exits.get(ward).copied().unwrap_or(0.0);
                Some((*ward, exits / hours * 24.0))
            })
            .collect();
        TallySnapshot {
            game_days: self.last_game_days,
            span_game_days: span,
            per_person_per_game_day: per_person,
            per_person_per_game_day_raw: per_person_raw,
            ward_exit_rate_per_game_day,
            mint_ward: self.mint_ward.clone(),
            warm_mint_ward_game_hours: self.warm_mint_ward_game_hours.clone(),
            holder_exits: self.holder_exits.clone(),
            holder_exits_same_trade: self.holder_exits_same_trade.clone(),
            warm_same_trade_exits: self.warm_same_trade_exits.clone(),
        }
    }
}

/// The ward whose air holds this fact at `hops: 0` with no `via` — the row
/// `install_fact` seeded, and therefore the ward it was minted in. See
/// [`TopicRow::mint_ward`] for the two writers of such a row and why the tally
/// caches this answer at its first sample.
fn minted_in(world: &World, key: FactKey) -> Option<PlanningWard> {
    PlanningWard::ALL.into_iter().find(|ward| {
        world
            .knowledge
            .drift(*ward, key)
            .is_some_and(|drift| drift.hops == 0 && drift.via.is_none())
    })
}

impl PollenCensus {
    /// The one-line header: the store, the roll bound, and X.
    pub fn summary(&self) -> String {
        format!(
            "t={:.4}gd facts {} holdings {} air {} store {:.1}KB rolls/gh(bound) {:.1} \
             X {:.2}/person/day (raw {:.2})",
            self.game_days,
            self.facts,
            self.holdings,
            self.air_entries,
            self.store_bytes as f64 / 1024.0,
            self.rolls_per_game_hour,
            self.crossings_per_person_per_game_day,
            self.crossings_per_person_per_game_day_raw,
        )
    }

    /// Every `[pollen]` line of one sample: the header, the eight wards, then one
    /// line per fact in [`Topic::ALL`] order — stable, so two runs diff cleanly
    /// and `--pollen-flat` can be compared to `--pollen-no-salience` with `diff`.
    ///
    /// Nothing here ever reaches a prompt, so `heat`, `hops` and `stir` may all be
    /// named: D14 governs `assets/prompts/`, not stdout.
    pub fn topic_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("[pollen] {}", self.summary())];
        let wards: Vec<String> = PlanningWard::ALL
            .iter()
            .map(|ward| {
                format!(
                    "{} {} c̄{:.3} X{:.2}",
                    ward.as_str(),
                    self.ward_population.get(ward).copied().unwrap_or(0),
                    self.ward_mean_curiosity.get(ward).copied().unwrap_or(0.0),
                    self.ward_exit_rate_per_game_day
                        .get(ward)
                        .copied()
                        .unwrap_or(0.0),
                )
            })
            .collect();
        lines.push(format!("[pollen] wards {}", wards.join(" | ")));
        for row in &self.topics {
            let by_ward: Vec<String> = row
                .carriers_by_ward
                .iter()
                .map(|(ward, count)| format!("{} {count}", ward.as_str()))
                .collect();
            lines.push(format!(
                "[pollen] {} {} mint {} wards {}/8 carriers {} warm {} expect {:.3} \
                 exits {:.0} (same {:.0}, warm-same-any {:.0}) hops {:.2} heat {:.3} \
                 age {:.2}gh by {}",
                row.topic.as_str(),
                row.fact.as_str(),
                row.mint_ward.map_or("-", PlanningWard::as_str),
                row.wards_reached,
                row.carriers,
                row.volunteering,
                row.expected_crossings,
                row.holder_exits,
                row.holder_exits_same_trade,
                row.warm_same_trade_exits,
                row.mean_hops,
                row.max_heat,
                row.age_game_hours,
                by_ward.join(" "),
            ));
        }
        lines
    }

    /// The tally-derived half: X (city and per ward), and per fact the model's own
    /// `expected_crossings = warm_mint_ward_game_hours × X_W / 24` beside the
    /// realised `holder_exits`. `X_W` is the exit rate of the ward the **tally**
    /// resolved as the mint ward, never the census's own reading of the air.
    pub fn fill(&mut self, tally: &TallySnapshot) {
        self.crossings_per_person_per_game_day = tally.per_person_per_game_day;
        self.crossings_per_person_per_game_day_raw = tally.per_person_per_game_day_raw;
        self.ward_exit_rate_per_game_day = tally.ward_exit_rate_per_game_day.clone();
        for (row, key) in self.topics.iter_mut().zip(self.keys.iter()) {
            row.holder_exits = tally.holder_exits.get(key).copied().unwrap_or(0.0);
            row.holder_exits_same_trade = tally
                .holder_exits_same_trade
                .get(key)
                .copied()
                .unwrap_or(0.0);
            row.warm_same_trade_exits =
                tally.warm_same_trade_exits.get(key).copied().unwrap_or(0.0);
            let warm = tally
                .warm_mint_ward_game_hours
                .get(key)
                .copied()
                .unwrap_or(0.0);
            let exit_rate = tally
                .mint_ward
                .get(key)
                .copied()
                .or(row.mint_ward)
                .and_then(|ward| tally.ward_exit_rate_per_game_day.get(&ward).copied())
                .unwrap_or(0.0);
            row.expected_crossings = warm * exit_rate / 24.0;
        }
    }

    /// The row for one fact, by its id — never by topic: the two shipped authored
    /// rows are `Coin` and `Craft` too, and they sort before a pack's, so a topic
    /// lookup would read `facts.json`'s stall quarrel where a test meant the pack.
    pub fn row_of_id(&self, id: &FactId) -> Option<&TopicRow> {
        self.topics.iter().find(|row| &row.fact == id)
    }
}

/// A census of one instant. **Pure: it never writes the store.**
///
/// [`TopicRow::wards_reached`] is the number of wards whose air holds a row for
/// the fact — deposits, never holders' wards (D54).
pub fn census(world: &World, clock: &WorldClock, now: f64) -> PollenCensus {
    let game_days = clock.game_days(now);
    let mut ward_population: BTreeMap<PlanningWard, usize> = BTreeMap::new();
    let mut curiosity_sum: BTreeMap<PlanningWard, f64> = BTreeMap::new();

    // One pass over the present cast: their ward, their curiosity, and — for each
    // live fact — whether they hold it and whether they are warm on it.
    // `Topic::ALL` order first, then `FactKey` — the order the print and the
    // `--pollen-flat` diff both rely on.
    let mut live: Vec<(FactKey, Topic, FactId, Option<f64>)> = world
        .knowledge
        .facts()
        .map(|(key, fact)| (key, fact.topic, fact.id.clone(), fact.minted_game_days))
        .collect();
    live.sort_by_key(|(key, topic, _, _)| (topic_ordinal(*topic), *key));
    let mut carriers: BTreeMap<FactKey, usize> = BTreeMap::new();
    let mut volunteering: BTreeMap<FactKey, usize> = BTreeMap::new();
    let mut hops_sum: BTreeMap<FactKey, u64> = BTreeMap::new();
    let mut max_heat: BTreeMap<FactKey, f32> = BTreeMap::new();
    let mut by_ward: BTreeMap<FactKey, BTreeMap<PlanningWard, usize>> = BTreeMap::new();
    let mut holdings = 0usize;
    let mut rolls = 0.0_f64;

    for (id, character) in &world.characters {
        if !world.is_present(id) {
            continue;
        }
        let Some(ward) = world.ward_at(character.position_m()) else {
            continue;
        };
        *ward_population.entry(ward).or_insert(0) += 1;
        *curiosity_sum.entry(ward).or_insert(0.0) += curiosity_for(world, id);
        rolls += world.knowledge.ward_air(ward).count() as f64;
        let rows = world.knowledge.rows_of(id);
        for (key, _, _, _) in &live {
            let Some(fact) = world.knowledge.fact(*key) else {
                continue;
            };
            // `holds_key`'s own order — seeded first, then stored — off rows
            // fetched once per body, so the census is O(bodies × live) lookups
            // of a `FactKey` map and not of the 20,520-key holdings map.
            let held = if fact.seeded.contains(id) {
                Held::seeded(fact)
            } else {
                match rows.binary_search_by(|row| row.key.cmp(key)) {
                    Ok(at) => rows[at].held(),
                    Err(_) => continue,
                }
            };
            holdings += 1;
            *carriers.entry(*key).or_insert(0) += 1;
            *hops_sum.entry(*key).or_insert(0) += u64::from(held.hops);
            *by_ward.entry(*key).or_default().entry(ward).or_insert(0) += 1;
            let heat = held.heat(Some(game_days));
            let slot = max_heat.entry(*key).or_insert(0.0);
            if heat > *slot {
                *slot = heat;
            }
            if crate::knowledge::volunteers(world, fact, id, &held, Some(game_days)) {
                *volunteering.entry(*key).or_insert(0) += 1;
            }
        }
    }

    let ward_mean_curiosity = curiosity_sum
        .iter()
        .map(|(ward, sum)| {
            let people = ward_population.get(ward).copied().unwrap_or(0).max(1);
            (*ward, sum / people as f64)
        })
        .collect();

    let mut topics = Vec::with_capacity(live.len());
    let mut keys = Vec::with_capacity(live.len());
    for (key, topic, fact_id, minted) in live {
        let wards_reached = PlanningWard::ALL
            .iter()
            .filter(|ward| world.knowledge.drift(**ward, key).is_some())
            .count() as u8;
        let held_by = carriers.get(&key).copied().unwrap_or(0);
        keys.push(key);
        topics.push(TopicRow {
            topic,
            fact: fact_id,
            mint_ward: minted_in(world, key),
            wards_reached,
            carriers: held_by,
            volunteering: volunteering.get(&key).copied().unwrap_or(0),
            expected_crossings: 0.0,
            holder_exits: 0.0,
            holder_exits_same_trade: 0.0,
            warm_same_trade_exits: 0.0,
            mean_hops: match held_by {
                0 => 0.0,
                count => hops_sum.get(&key).copied().unwrap_or(0) as f64 / count as f64,
            },
            max_heat: max_heat.get(&key).copied().unwrap_or(0.0),
            age_game_hours: minted.map_or(0.0, |at| ((game_days - at) * 24.0).max(0.0)),
            carriers_by_ward: by_ward.remove(&key).unwrap_or_default(),
        });
    }

    PollenCensus {
        game_days,
        facts: world.knowledge.len(),
        holdings,
        air_entries: world.knowledge.air_entries(),
        store_bytes: world.knowledge.footprint_bytes(),
        rolls_per_game_hour: rolls * STIRS_PER_GAME_HOUR,
        crossings_per_person_per_game_day: 0.0,
        crossings_per_person_per_game_day_raw: 0.0,
        ward_exit_rate_per_game_day: BTreeMap::new(),
        ward_population,
        ward_mean_curiosity,
        topics,
        keys,
    }
}

/// [`Topic::ALL`]'s index, so a sort by topic is the print's own order and not
/// `Ord`'s.
fn topic_ordinal(topic: Topic) -> usize {
    Topic::ALL
        .iter()
        .position(|other| *other == topic)
        .unwrap_or(Topic::ALL.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Character;
    use crate::knowledge::{
        GarbleMask, HEAT_GONE_BELOW, HOP_LOSS, POLLEN_POLL_MAX_GAME_MINUTES, Topic, VOLUNTEER_HEAT,
    };
    use crate::lore::{LoreProfile, Significance};
    use std::collections::{BTreeMap, BTreeSet};

    const NAV_JSON: &str = include_str!("../../../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../../../assets/world/navigation.bin");

    /// One fact, built here rather than through the loader: `pollen` is a child of
    /// `knowledge`, so the private `source` field is in reach and a test needs no
    /// JSON to ask a question about a number.
    fn fact_of(topic: Topic, subject: &[&str], craft_ear: Option<&str>) -> Fact {
        Fact {
            id: crate::ids::FactId::from_raw("test.row"),
            key: crate::ids::FactKey(0),
            sequence: 7,
            subject: subject.iter().map(|id| ActorId::from_raw(*id)).collect(),
            place: None,
            day: None,
            said: "{subject} did a thing".into(),
            own: BTreeMap::new(),
            seeded: BTreeSet::new(),
            garble: GarbleMask::NONE,
            decays: true,
            topic,
            minted_game_days: Some(0.0),
            quiet_among: BTreeSet::new(),
            craft_ear: craft_ear.map(str::to_string),
            source: crate::knowledge::FactSource::authored(),
        }
    }

    fn lore_with(occupation: Option<&str>, curiosity: Option<f64>) -> LoreProfile {
        LoreProfile {
            significance: Significance::Minor,
            planning_ward: PlanningWard::Wick,
            age: 30,
            gender: "f".into(),
            occupation_id: occupation.map(str::to_string),
            occupation_display: occupation.map(str::to_string),
            title: None,
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "Wick".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: None,
            home_point_m: None,
            core_character_description: String::new(),
            extended_character_description: String::new(),
            curiosity,
            generated: false,
        }
    }

    fn person(id: &str, lore: Option<LoreProfile>) -> Character {
        person_under(id, lore, crate::character::Control::Llm)
    }

    fn person_under(
        id: &str,
        lore: Option<LoreProfile>,
        control: crate::character::Control,
    ) -> Character {
        Character::from_sheet(crate::character::CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw(id),
            name: id.to_string(),
            control,
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
            lore,
            presence: crate::character::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::character::EconomicClass::Resident,
        })
    }

    /// The grid is the size the derivation says, so a cell-size change is a
    /// deliberate one and never a side effect.
    #[test]
    fn the_ward_grid_is_the_size_it_says() {
        assert_eq!(WARD_CELL_M, 8.0);
        let grid = ward_grid();
        assert_eq!(grid.cells(), 9_464, "91 × 104 cells over the walkable box");
        assert_eq!(
            grid.ambiguous_cells(),
            508,
            "5.37% of cells straddle a ward boundary and fall through to the exact search"
        );
    }

    /// What the standing wards actually look like on the ground — the geometry
    /// every crossing figure is a property of. A Voronoi over 321 doors labelled by
    /// ward is not eight compact blobs: a street with one ward's doors on one side
    /// and another's on the other is a boundary the length of the street. Printed
    /// for `m2_measurements.md`: pieces per ward (4-connected over settled cells),
    /// the share of settled cells with a foreign settled neighbour, and how far the
    /// Wickmarket stands from the nearest ground that is not Wick's — the `d_b` the
    /// slow end was solved against (D53, ≈ 82 m).
    #[test]
    fn the_standing_wards_are_a_patchwork() {
        let grid = ward_grid();
        let (cols, rows) = (grid.cols, grid.rows);
        let cell = |cx: usize, cz: usize| grid.cells[cz * cols + cx];
        let mut pieces: BTreeMap<PlanningWard, usize> = BTreeMap::new();
        let mut seen = vec![false; cols * rows];
        let mut border = 0usize;
        let mut settled = 0usize;
        for cz in 0..rows {
            for cx in 0..cols {
                let here = cell(cx, cz);
                if here == AMBIGUOUS {
                    continue;
                }
                settled += 1;
                let neighbours = [
                    (cx.wrapping_sub(1), cz),
                    (cx + 1, cz),
                    (cx, cz.wrapping_sub(1)),
                    (cx, cz + 1),
                ];
                if neighbours.iter().any(|&(nx, nz)| {
                    nx < cols && nz < rows && cell(nx, nz) != AMBIGUOUS && cell(nx, nz) != here
                }) {
                    border += 1;
                }
                if seen[cz * cols + cx] {
                    continue;
                }
                // Flood one piece.
                let mut stack = vec![(cx, cz)];
                seen[cz * cols + cx] = true;
                while let Some((x, z)) = stack.pop() {
                    for (nx, nz) in [
                        (x.wrapping_sub(1), z),
                        (x + 1, z),
                        (x, z.wrapping_sub(1)),
                        (x, z + 1),
                    ] {
                        if nx < cols && nz < rows && !seen[nz * cols + nx] && cell(nx, nz) == here {
                            seen[nz * cols + nx] = true;
                            stack.push((nx, nz));
                        }
                    }
                }
                *pieces
                    .entry(PlanningWard::ALL[usize::from(here)])
                    .or_insert(0) += 1;
            }
        }
        let wickmarket = Vec3::new(-17.375, 0.91, 248.375);
        let home = grid.at(wickmarket);
        let mut nearest_foreign = f64::INFINITY;
        for step in 0..200_000u64 {
            let mut hasher = DefaultHasher::new();
            ("patchwork_probe", step).hash(&mut hasher);
            let bits = hasher.finish();
            let dx = -300.0 + f64::from((bits >> 32) as u32) / f64::from(u32::MAX) * 600.0;
            let dz = -300.0 + f64::from(bits as u32) / f64::from(u32::MAX) * 600.0;
            let point = Vec3::new(wickmarket.x + dx, 0.91, wickmarket.z + dz);
            if grid.at(point) != home {
                nearest_foreign = nearest_foreign.min((dx * dx + dz * dz).sqrt());
            }
        }
        let pieces_line: Vec<String> = pieces
            .iter()
            .map(|(ward, count)| format!("{} {count}", ward.as_str()))
            .collect();
        println!(
            "[pollen] the standing wards on the ground: pieces per ward {} ({} in all); \
             {:.1}% of settled cells border another ward; the Wickmarket stands {:?} and the \
             nearest ground of another ward is {nearest_foreign:.0} m away",
            pieces_line.join(", "),
            pieces.values().sum::<usize>(),
            border as f64 / settled.max(1) as f64 * 100.0,
            home,
        );
        assert_eq!(
            home,
            Some(PlanningWard::Wick),
            "the Wickmarket stands in Wick"
        );
        assert!(
            (60.0..110.0).contains(&nearest_foreign),
            "the slow end was solved against a nearest foreign boundary of ≈ 82 m from the \
             Wickmarket (D53); it is now {nearest_foreign:.0} m — re-solve VOLUNTEER_HEAT \
             from it (02_numbers.md §10.2)"
        );
    }

    /// The whole claim of D23 in one loop: the grid is not an approximation of the
    /// nearest-mark search, it **is** it — inside the city box, on its edges and
    /// far outside it, where every query falls through.
    #[test]
    fn the_ward_grid_matches_the_exact_search() {
        let grid = ward_grid();
        for step in 0..10_000u64 {
            let mut hasher = DefaultHasher::new();
            ("ward_grid_probe", step).hash(&mut hasher);
            let bits = hasher.finish();
            let x = -500.0 + f64::from((bits >> 32) as u32) / f64::from(u32::MAX) * 1000.0;
            let z = -600.0 + f64::from(bits as u32) / f64::from(u32::MAX) * 1100.0;
            let point = Vec3::new(x, 0.91, z);
            assert_eq!(
                grid.at(point),
                grid.exact_at(point),
                "the grid disagreed with the exact search at ({x}, {z})"
            );
        }
    }

    /// A hand-built nav has no `homes.json` behind it, so nobody has a ward — and
    /// the answer must be "nothing", never a wrong ward, which is the case
    /// `crowd::doorsteps` already guards.
    #[test]
    fn an_empty_ward_map_answers_nothing_rather_than_a_wrong_ward() {
        let grid = WardGrid {
            cells: vec![AMBIGUOUS; 4],
            cols: 2,
            rows: 2,
            marks: Vec::new(),
        };
        assert!(grid.is_empty());
        for point in [
            Vec3::new(0.0, 0.91, 0.0),
            Vec3::new(-17.375, 0.91, 248.375),
            Vec3::new(5_000.0, 0.91, -5_000.0),
        ] {
            assert_eq!(grid.at(point), None);
            assert_eq!(grid.exact_at(point), None);
        }
    }

    /// Step 6's acceptance check, structurally: the ground a generated citizen is
    /// actually stood on answers the same ward it answered before `crowd.rs` was
    /// routed through the grid — because on that ground the grid *is* the exact
    /// search it replaced.
    #[test]
    fn the_ward_at_answer_never_changes_for_a_generated_citizen() {
        let nav = crate::nav::NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav");
        let grid = ward_grid();
        let stands = crate::crowd::spread_over_walkable(&nav, 1_000);
        assert!(stands.len() >= 900, "the crowd stands on the shipped graph");
        for point in stands {
            assert_eq!(
                grid.at(point),
                grid.exact_at(point),
                "a citizen stood at ({}, {}) would change ward",
                point.x,
                point.z
            );
        }
    }

    /// The invariant that makes the roll rate a constant instead of a function of
    /// the jitter: nobody can skip a stir window, at any clock this game runs at.
    #[test]
    fn the_poll_gap_cannot_skip_a_stir() {
        let window = 60.0 / STIRS_PER_GAME_HOUR;
        // Bare, and then composed with every host step: `--watch-clock`'s default
        // (`seconds_per_day / 200` real seconds = 7.2 game minutes at any clock),
        // `--trace-pollen`'s 0.4 s cadence step at 3600 s/day, and one round tick
        // under the `T` key's 60×.
        for (label, step) in [
            ("bare", 0.0),
            ("the default watch step", 7.2),
            ("the 0.4 s cadence step at 3600 s/day", 0.16),
            ("one round tick at 60x", 1.2),
        ] {
            assert!(
                POLLEN_POLL_MAX_GAME_MINUTES + step < window,
                "a person could skip a whole stir window with {label}: \
                 {POLLEN_POLL_MAX_GAME_MINUTES} + {step} game minutes against \
                 60 / STIRS_PER_GAME_HOUR = {window}. The effective roll rate would stop \
                 being S and every closed form in 02_numbers.md would stop being solvable"
            );
        }
    }

    /// Both brackets `HOP_LOSS` sits inside, with the reason each one is there.
    ///
    /// Comparing constants is the whole point — it is a contract between two numbers
    /// in separate places, not a tautology — so `assertions_on_constants` is allowed
    /// here exactly as it is on the store's own bracket test.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_hop_loss_is_bracketed() {
        assert!(
            HOP_LOSS.powi(4) > VOLUNTEER_HEAT,
            "a fourth-hand top-band story must still be repeatable, or the ladder's far \
             rungs are unreachable: {} against {VOLUNTEER_HEAT}",
            HOP_LOSS.powi(4)
        );
        assert!(
            HOP_LOSS * 0.12 < VOLUNTEER_HEAT,
            "an off-affinity trade matter must travel one hop and stop: {} against \
             {VOLUNTEER_HEAT}",
            HOP_LOSS * 0.12
        );
    }

    /// Never 0.0: exponential decay underflows to exactly zero and `0.0 < 0.0` is
    /// false forever, which makes the row immortal and holds a cap slot nothing can
    /// reclaim.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_air_heat_floor_is_strictly_positive() {
        assert!(HEAT_GONE_BELOW > 0.0);
        assert!(
            HEAT_GONE_BELOW < 0.05,
            "a floor this high would evict live news"
        );
    }

    /// A garbage chance is silence, not a coin flip — the guard `notices::carries`
    /// lacks and `custody::struggle_roll` has.
    #[test]
    fn a_garbage_chance_is_silence_not_a_coin_flip() {
        let fact = fact_of(Topic::Bed, &["subjct"], None);
        let listener = ActorId::from_raw("hearer");
        for chance in [f64::NAN, -1.0, 0.0] {
            assert!(
                !picks_up(&fact, &listener, 3, chance),
                "{chance} is silence"
            );
        }
        for chance in [1.0, 2.0] {
            assert!(picks_up(&fact, &listener, 3, chance), "{chance} is certain");
        }
    }

    /// The roll is a hash of stable inputs: idempotent inside a stir window, and a
    /// new coin on the next one. A fresh draw per poll would make a 1-in-20 a
    /// certainty inside a frame.
    #[test]
    fn the_pickup_roll_is_stable_within_a_stir_and_moves_across_one() {
        let fact = fact_of(Topic::Bed, &["subjct"], None);
        let listener = ActorId::from_raw("hearer");
        let first = picks_up(&fact, &listener, 11, 0.5);
        for _ in 0..1_000 {
            assert_eq!(picks_up(&fact, &listener, 11, 0.5), first);
        }
        let mut moved = false;
        for index in 0..100 {
            let who = ActorId::from_raw(format!("p{index:04}"));
            if picks_up(&fact, &who, 11, 0.5) != picks_up(&fact, &who, 12, 0.5) {
                moved = true;
                break;
            }
        }
        assert!(moved, "the next stir must be a new coin for somebody");
    }

    /// The clamp in [`pickup_chance`] never activates, which is what keeps the roll
    /// linear and every retune a division rather than a search.
    ///
    /// The whole-cast walk lives in `cathedral-backends`' `pollen_cadence.rs`,
    /// which is the only crate that can read `lore/characters/**`; what is checked
    /// here is the arithmetic it rests on — the table's two worst rows against the
    /// measured maximum curiosity in the cast (0.392, `p00au`, no trade).
    #[test]
    fn the_pickup_chance_never_clamps() {
        let mut world = World::new();
        const MAX_CAST_CURIOSITY: f64 = 0.392;
        // The no-trade quarter hears everything (×1.4) and holds the cast's highest
        // curiosity; the ×1.6 `Bed` ear is occupation-based, so nobody gets both.
        world.add_character(person(
            "pauper",
            Some(lore_with(None, Some(MAX_CAST_CURIOSITY))),
        ));
        world.add_character(person(
            "servnt",
            Some(lore_with(Some("domestic_servant"), Some(0.292))),
        ));
        world.add_character(person("subjct", Some(lore_with(Some("cooper"), None))));
        let mut worst = 0.0_f64;
        for topic in Topic::ALL {
            let fact = fact_of(topic, &["subjct"], Some("cooper"));
            for who in ["pauper", "servnt"] {
                let chance = pickup_chance(&world, &fact, &ActorId::from_raw(who), 1.0);
                assert!(
                    chance < 1.0,
                    "{topic:?} × {who} clamps at {chance}; the roll would stop being linear"
                );
                worst = worst.max(chance);
            }
        }
        println!("[pollen] worst per-roll chance in the table: {worst:.3}");
        assert!(
            (worst - 0.549).abs() < 0.002,
            "the worst per-roll chance moved to {worst}; 02_numbers.md §4 says 0.549"
        );
    }

    /// The subject never rolls, whatever the table says and under either
    /// measurement lever — the rule sits outside the product.
    #[test]
    fn the_subject_never_rolls_under_either_lever() {
        let mut world = World::new();
        world.add_character(person("subjct", Some(lore_with(Some("cooper"), None))));
        let fact = fact_of(Topic::Bed, &["subjct"], None);
        let subject = ActorId::from_raw("subjct");
        assert_eq!(pickup_chance(&world, &fact, &subject, 1.0), 0.0);
        world.salience = std::sync::Arc::new(crate::knowledge::SalienceTable::flat());
        assert_eq!(pickup_chance(&world, &fact, &subject, 1.0), 0.0);
        world.pollen_no_salience = true;
        assert_eq!(pickup_chance(&world, &fact, &subject, 1.0), 0.0);
    }

    /// A row starts on the current coin, never on coin 0.
    #[test]
    fn a_row_starts_on_the_current_coin() {
        assert_eq!(stage_stir(0.0), 0);
        assert_eq!(stage_stir(0.5), 24, "half a game day is 24 stirs");
        assert_eq!(
            stage_stir(-3.0),
            0,
            "a negative clock is coin 0, never a panic"
        );
    }

    /// One installed fact, and one ward with it in the air at heat 1.0.
    fn air_world(topic: Topic, game_days: f64) -> (World, crate::ids::FactKey, PlanningWard) {
        let mut world = World::new();
        let (handle, sequence) = world.knowledge.next_handles();
        let mut fact = fact_of(topic, &["subjct"], None);
        fact.key = handle;
        fact.sequence = sequence;
        let key = world.knowledge.install(fact).expect("the row installs");
        let ward = PlanningWard::Fabric;
        world.knowledge.seed_air(ward, key, 1.0, game_days);
        (world, key, ward)
    }

    /// Heat is quantised **before** the change test, so a sweep that cooled the air
    /// by nine decimal places of nothing reports nothing.
    ///
    /// One cooling step multiplies heat by ~0.944, twelve orders of magnitude above
    /// `f32::EPSILON`, so a raw-`f32` `!=` would be true on every sweep and would
    /// churn the diagnostic chain forever. Half a game hour is the stir grid's own
    /// step, so the previous sweep goes a hair below a grid edge and this one a hair
    /// above it: the beat fires, and what it charges is nothing a whole percent
    /// could hold.
    #[test]
    fn the_sweep_quantises_before_deciding_anything_changed() {
        let edge = 1.0 / 48.0;
        // A thousandth of a game hour: `f32` sees it (λ^0.001 = 0.999942, four
        // hundred times f32's epsilon) and a whole percent cannot.
        let hair = 0.001 / 24.0;
        let (mut world, key, ward) = air_world(Topic::Bed, 0.0);
        world.knowledge.rewind_sweep_clock(edge - hair / 2.0);
        assert!(
            !sweep(&mut world, edge + hair / 2.0),
            "two sweeps a hair apart must report nothing, or the change test churns"
        );
        let air = world.knowledge.drift(ward, key).expect("the row").clone();
        assert_eq!(heat_pct(air.heat), 100);
        assert!(
            air.heat < 1.0,
            "the row did cool — by the hair a raw f32 comparison would have reported"
        );
        assert_eq!(air.stir, stage_stir(0.0) + 1, "the beat is still a coin");

        // A real half-hour step moves a whole percent, and says so.
        world.knowledge.rewind_sweep_clock(edge);
        assert!(sweep(&mut world, edge * 2.0));
        assert_eq!(
            heat_pct(world.knowledge.drift(ward, key).expect("the row").heat),
            97,
            "half a game hour is a factor of lambda^0.5 = 0.9716"
        );
    }

    /// The coin is a function of game time: one sweep that spans three grid edges
    /// mints three coins, not one, so a stall longer than a stir window (75 s at
    /// the shipped clock, 1.25 s at the `T` key's 60×) cannot quietly lower the
    /// roll rate below `STIRS_PER_GAME_HOUR`. Cooling was already exact; this
    /// makes the coin match it.
    #[test]
    fn a_sweep_across_three_edges_mints_three_coins() {
        let edge = 1.0 / 48.0;
        let (mut world, key, ward) = air_world(Topic::Bed, 0.0);
        let coin = world.knowledge.drift(ward, key).expect("the row").stir;
        world.knowledge.rewind_sweep_clock(0.0);
        assert!(sweep(&mut world, 3.0 * edge + edge / 4.0));
        let air = world.knowledge.drift(ward, key).expect("the row").clone();
        assert_eq!(air.stir, coin + 3, "three grid edges elapsed, three coins");
        assert_eq!(
            heat_pct(air.heat),
            heat_pct(super::super::cooling_lambda().powf(1.625) as f32),
            "and the cooling charged the exact 1.625 game hours"
        );
        // One edge is still one coin.
        world.knowledge.rewind_sweep_clock(3.0 * edge + edge / 4.0);
        assert!(sweep(&mut world, 4.0 * edge + edge / 4.0));
        assert_eq!(
            world.knowledge.drift(ward, key).expect("the row").stir,
            coin + 4
        );
    }

    /// The player is rolled at [`PLAYER_CURIOSITY`] and is **not** the no-trade
    /// quarter, so their whole roll is that constant — while a lore-less NPC keeps
    /// `curiosity_of`'s 1.0 fallback and the ×1.4 that would have handed the player
    /// the entire ward's air on their first poll.
    #[test]
    fn the_player_rolls_at_their_own_curiosity() {
        let mut world = World::new();
        world.add_character(person_under(
            "player",
            None,
            crate::character::Control::Player,
        ));
        world.add_character(person("hearer", None));
        world.add_character(person("subjct", Some(lore_with(Some("cooper"), None))));
        let fact = fact_of(Topic::Bed, &["subjct"], None);
        let chance = pickup_chance(&world, &fact, &ActorId::from_raw("player"), 1.0);
        assert!(
            (chance - PLAYER_CURIOSITY).abs() < 1e-12,
            "the player's roll is {chance}, not PLAYER_CURIOSITY"
        );
        assert_eq!(
            pickup_chance(&world, &fact, &ActorId::from_raw("hearer"), 1.0),
            1.0,
            "a lore-less NPC clamps: CURIOSITY_WITHOUT_LORE x no_trade is over one,              which is exactly what the player must not be rolled at"
        );
    }
}
