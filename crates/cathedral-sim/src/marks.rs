//! Chalk on the walls — stigmergy the city can read
//! (`features/chalking_the_walls.md`).
//!
//! A mark is the environment used as a database. Rule-driven hands chalk it
//! (a cross on a debtor's door, a tally at a well, a ward-sign at a place of
//! resort) and cheap rules read it back, and **nothing that reads a mark ever
//! asks who drew it** — so a forged cross refuses a stall exactly as hard as
//! the ward's own, and scrubbing one is the player tampering with the medium
//! instead of with anybody's mind.
//!
//! The whole loop is zero-token: marks are written by code, read by code, and
//! reach an LLM only as one already-paid-for line on a turn that was going to
//! happen anyway.
//!
//! Three decisions are load-bearing enough to restate here, because breaking
//! any of them silently un-builds the feature:
//!
//! 1. **A mark is authoritative, never a projection.** The stall refusal reads
//!    the chalk and never [`World::notices`](crate::world::World::notices). If
//!    marks merely mirrored the notices, scrubbing would change nothing and
//!    forging would be placebo.
//! 2. **No reader may branch on [`Mark::author`].** It is recorded for the
//!    trace and for a later "mute witness" feature. `if mark.author.is_none()`
//!    anywhere in a reader is a bug.
//! 3. **A mark anchors to a handle the city already has** — a household or a
//!    registered place — never to a free coordinate. That is what keeps the
//!    sim free of new geometry, keeps every reader a lookup rather than a
//!    space sweep, and makes the render trivial. An anchor that stops
//!    resolving takes its mark with it, quietly, on the next sweep.
//!
//! Marks are *slow* state, unlike the dogs' per-frame poses: they ride the
//! cold [`PublicSnapshot`](crate::snapshot::PublicSnapshot) and bump the world
//! revision, rather than getting a hot channel of their own.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ActorId, MarkId};
use crate::math::Vec3;
use crate::world::World;

/// The authored catalog. Embedded (`include_str!`) exactly as `items.json`
/// (`item.rs`) and `places.json` (`places.rs`) are, so both hosts and the
/// headless runner get it with no wiring and there is no way for the game and
/// the terminal to disagree about what a cross means.
const MARKS_JSON: &str = include_str!("../../../assets/world/marks.json");

/// The hard cap on live marks. **Measured, not chosen**: the public snapshot
/// is 137,025 bytes against a 160 KiB assertion
/// (`crates/cathedral-backends/src/world_data.rs`), leaving 26,815 bytes. A
/// quantized [`PublicMark`](crate::snapshot::PublicMark) measures **100.1
/// bytes** on the wire at worst case, so a fully chalked city adds 10,010
/// bytes and leaves 16,805 for everything that comes after.
///
/// The spec suggested 256. Measured, 256 *does* fit — 162,635 bytes — but
/// only by 1,205, which is to say it spends 96% of the headroom the project
/// has left on chalk and leaves the next feature nothing. 100 is the number
/// because it is the one that leaves room, not because 256 overflows.
///
/// Those are not estimates — `full_roster_prompts_and_public_snapshot_remain_bounded`
/// fills the walls to this cap and re-asserts the bound, and prints both
/// numbers. Raise this and that test tells you what it cost.
///
/// A player with a pen and an afternoon must not be able to grow world state
/// without limit, so drawing at the cap evicts the faintest mark.
pub const MARKS_MAX: usize = 100;

/// How near an actor's *anchor site* must be for a mark to reach their sheet.
pub const MARK_NOTICE_RADIUS_M: f64 = 8.0;

/// A tally saturates here: a well everybody used all day reads "very busy",
/// not an unbounded number, and the reader's penalty stays bounded with it.
pub const TALLY_STROKES_MAX: u32 = 12;

/// Decay is swept at most this many times a game day — once a game-minute.
/// The engine polls at ~60 Hz; sweeping every poll would be 3,600× the work
/// for a quantity that moves in days.
const SWEEPS_PER_GAME_DAY: f64 = 1440.0;

// --------------------------------------------------------------------------- //
// The model
// --------------------------------------------------------------------------- //

/// What a mark says. The vocabulary lives in the *kinds*, not in a spread of
/// materials — everything is chalk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    /// On a debtor's door. Read by the stall counter, which refuses the sale.
    ChalkCross,
    /// At a water source, one stroke per draw. Read by the thirsty, who go
    /// elsewhere.
    WellTally,
    /// At a ward's place of resort. Read by the ambient cast's evening roll.
    WardSign,
}

impl MarkKind {
    /// The catalog key, and the handle a verb argument spells.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChalkCross => "chalk_cross",
            Self::WellTally => "well_tally",
            Self::WardSign => "ward_sign",
        }
    }

    /// Resolve a verb argument or a drive handle. Unknown kinds are a plain
    /// `None` — the caller turns that into the `unknown_kind` error.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "chalk_cross" => Some(Self::ChalkCross),
            "well_tally" => Some(Self::WellTally),
            "ward_sign" => Some(Self::WardSign),
            _ => None,
        }
    }
}

impl std::fmt::Display for MarkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The per-kind ablation switches (`config.ron: smart_actors.marks`).
///
/// A kind switched off is written by nobody and read by nobody; marks of that
/// kind already on the walls are left alone and go on weathering, exactly as
/// the whole-layer switch behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkKindSwitches {
    pub cross: bool,
    pub tally: bool,
    pub ward_sign: bool,
}

impl Default for MarkKindSwitches {
    fn default() -> Self {
        Self {
            cross: true,
            tally: true,
            ward_sign: true,
        }
    }
}

impl MarkKindSwitches {
    pub fn enabled(&self, kind: MarkKind) -> bool {
        match kind {
            MarkKind::ChalkCross => self.cross,
            MarkKind::WellTally => self.tally,
            MarkKind::WardSign => self.ward_sign,
        }
    }
}

/// Which handle a mark hangs on. Never a free coordinate — see the module doc.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkAnchor {
    /// That person's home door. Resolves through
    /// [`PlaceRegistry::home_of`](crate::places::PlaceRegistry::home_of) —
    /// *not* through `homes.rs`, which has no per-actor lookup.
    Household(ActorId),
    /// A registered place: a well, a market, a chapel. Resolves through
    /// [`PlaceRegistry::named`](crate::places::PlaceRegistry::named).
    Place(String),
}

impl MarkAnchor {
    /// Which catalog `anchors` entry this variant answers to.
    fn slot(&self) -> AnchorSlot {
        match self {
            Self::Household(_) => AnchorSlot::Household,
            Self::Place(_) => AnchorSlot::Place,
        }
    }
}

/// The anchor variants a kind may hang on, as the catalog spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorSlot {
    Household,
    Place,
}

/// One live mark.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    pub anchor: MarkAnchor,
    /// Who the mark is *about*, derived from the anchor when it is drawn and
    /// never passed in by a caller. A household's cross is about its occupant;
    /// a place's tally is about nobody.
    pub about: Option<ActorId>,
    /// Whose hand. `None` is the ward's own — a system writer needs no
    /// inventory, exactly as `raise_notice` needs no parchment.
    ///
    /// **Never read by a reader.** See the module doc.
    pub author: Option<ActorId>,
    /// Absolute game-days when the chalk went on. Kept for the trace and for
    /// "how old is this", never for the decay arithmetic — see
    /// [`Mark::last_decayed_game_days`].
    pub drawn_game_days: f64,
    /// The decay clock's own anchor. Distinct from `drawn_game_days` because a
    /// mark drawn *between* two sweeps must not be charged for the whole
    /// interval, and because a re-chalk resets strength without resetting age.
    pub last_decayed_game_days: f64,
    /// `1.0` fresh, falling toward zero as it weathers.
    pub strength: f64,
    /// Tally notches. Every other kind stays at 1.
    pub strokes: u32,
}

/// Where a mark actually is, once its anchor has been resolved against the
/// world. Deliberately a value, not a borrow: the sweep resolves every site
/// under one immutable borrow and then mutates.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorSite {
    pub point: Vec3,
    /// The registry's own name for the site: `"Chain Well"`, or — for a
    /// household — `"Ede Clove's house"`.
    ///
    /// **Never render this to an actor without a `knows` check.** A household
    /// entry spells its owner, so putting this straight on a sheet would tell
    /// a stranger a name they were never told. The sheet uses
    /// [`AnchorSite::occupant`] and the unknown-people rule instead; this
    /// label is for places, the HUD and the trace.
    pub label: String,
    /// The household's occupant, when the anchor is one. The safe handle: the
    /// caller decides how to spell them.
    pub occupant: Option<ActorId>,
}

/// The live chalk. A `BTreeMap` so iteration order — and therefore every
/// prompt bullet, every snapshot and every eviction — is identical on a
/// replay.
#[derive(Debug, Clone, PartialEq)]
pub struct Marks {
    live: BTreeMap<MarkId, Mark>,
    next_id: u64,
    /// The last game-day the decay sweep ran, so the sweep can both gate
    /// itself to once a game-minute and charge the exact elapsed span.
    last_sweep_game_days: f64,
    /// Multiplies elapsed time in the decay. `1.0` in the game; a test or
    /// `config.ron` raises it to weather a wall in seconds.
    pub decay_scale: f64,
    /// The last game-day the ward's chalking beat ran, so a writer that is
    /// *called* every poll only *draws* once a day. See
    /// [`take_daily_beat`](Self::take_daily_beat).
    last_beat_game_days: f64,
}

impl Default for Marks {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            next_id: 0,
            last_sweep_game_days: f64::NEG_INFINITY,
            decay_scale: 1.0,
            last_beat_game_days: f64::NEG_INFINITY,
        }
    }
}

impl Marks {
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn get(&self, id: MarkId) -> Option<&Mark> {
        self.live.get(&id)
    }

    pub fn get_mut(&mut self, id: MarkId) -> Option<&mut Mark> {
        self.live.get_mut(&id)
    }

    /// Every live mark, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (MarkId, &Mark)> {
        self.live.iter().map(|(id, mark)| (*id, mark))
    }

    /// The first live mark of `kind` on `anchor`, if any. The idempotence test
    /// every writer runs before drawing.
    pub fn find(&self, kind: MarkKind, anchor: &MarkAnchor) -> Option<(MarkId, &Mark)> {
        self.live
            .iter()
            .find(|(_, mark)| mark.kind == kind && &mark.anchor == anchor)
            .map(|(id, mark)| (*id, mark))
    }

    /// Insert a mark, allocating its id. At [`MARKS_MAX`] this evicts the
    /// faintest mark first (ties break to the lowest id, so it is
    /// deterministic) and returns which id went, for the caller to log.
    pub fn insert(&mut self, mark: Mark) -> (MarkId, Option<MarkId>) {
        let evicted = if self.live.len() >= MARKS_MAX {
            let faintest = self
                .live
                .iter()
                .min_by(|(left_id, left), (right_id, right)| {
                    left.strength
                        .partial_cmp(&right.strength)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left_id.cmp(right_id))
                })
                .map(|(id, _)| *id);
            if let Some(id) = faintest {
                self.live.remove(&id);
            }
            faintest
        } else {
            None
        };
        self.next_id += 1;
        let id = MarkId(self.next_id);
        self.live.insert(id, mark);
        (id, evicted)
    }

    pub fn remove(&mut self, id: MarkId) -> Option<Mark> {
        self.live.remove(&id)
    }

    /// True at most once per game day — the ward's chalking beat.
    ///
    /// The engine polls at ~60 Hz, and the cross writer runs on every one of
    /// those polls. Without this gate it would refresh every live cross back
    /// to full strength 60 times a second, which would make scrubbing a cross
    /// *completely* pointless: the sergeant would redraw it before the player
    /// straightened up. Gated, a scrub buys exactly what the spec says it
    /// should — a day.
    pub fn take_daily_beat(&mut self, game_days: f64) -> bool {
        if self.last_beat_game_days.is_finite()
            && game_days.floor() <= self.last_beat_game_days.floor()
        {
            return false;
        }
        self.last_beat_game_days = game_days;
        true
    }

    /// Test seam: pretend the beat last ran `game_days` ago.
    #[doc(hidden)]
    pub fn rewind_beat_clock(&mut self, game_days: f64) {
        self.last_beat_game_days = game_days;
    }

    /// Test and host seam: force the next [`sweep`] to run and to charge
    /// `elapsed` game-days. The game never calls this — the sweep's own clock
    /// gate is what paces it.
    #[doc(hidden)]
    pub fn rewind_sweep_clock(&mut self, game_days: f64) {
        self.last_sweep_game_days = game_days;
        for mark in self.live.values_mut() {
            mark.last_decayed_game_days = game_days;
        }
    }
}

// --------------------------------------------------------------------------- //
// The catalog
// --------------------------------------------------------------------------- //

/// Everything a kind means and how it weathers. Authored in
/// `assets/world/marks.json`; no meaning and no number is spelled in Rust.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkKindSpec {
    /// What a reader sees: "a chalk cross at knee height".
    pub label: String,
    /// The plain sentence everyone over seven knows. There is no literacy
    /// state and no unlock — a mark's meaning is legible to anyone who looks.
    pub meaning: String,
    /// The label once the mark is below [`MarkKindSpec::faint_below`].
    pub faint_label: String,
    pub anchors: Vec<AnchorSlot>,
    pub half_life_days_dry: f64,
    pub half_life_days_wet: f64,
    /// A sheltered anchor divides the precipitation it feels by this.
    pub sheltered_multiplier: f64,
    /// Below this the mark still renders, qualified, but its rule stops
    /// applying: a faint mark is a fact about the past, not a rule.
    pub faint_below: f64,
    /// Below this the mark is removed. Silently — chalk washing off is not
    /// news, and nobody's inbox hears about it.
    pub gone_below: f64,
    /// Whether `draw_mark` may produce it at all.
    pub drawable_by_hand: bool,
    /// Ward key → the place of resort a ward-sign may name. Only `ward_sign`
    /// carries one; see the catalog's own `_places_doc` for why the vocabulary
    /// is authored here rather than read off `places.json`.
    #[serde(default)]
    pub places: BTreeMap<String, String>,
    #[serde(default, rename = "_places_doc")]
    _places_doc: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarksDoc {
    schema_version: u32,
    #[serde(default, rename = "_doc")]
    _doc: Option<String>,
    #[serde(default, rename = "_decay_doc")]
    _decay_doc: Option<String>,
    kinds: BTreeMap<MarkKind, MarkKindSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkCatalog {
    kinds: BTreeMap<MarkKind, MarkKindSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkCatalogError {
    pub message: String,
}

impl std::fmt::Display for MarkCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Default for MarkCatalog {
    /// The embedded catalog. Panics only if the *compiled-in* asset is
    /// malformed, which is a build-time fact, not a runtime one — the same
    /// bargain `item.rs` makes.
    fn default() -> Self {
        Self::from_embedded().expect("the embedded mark catalog must parse and validate")
    }
}

impl MarkCatalog {
    pub fn from_embedded() -> Result<Self, MarkCatalogError> {
        Self::from_json(MARKS_JSON)
    }

    pub fn from_json(json: &str) -> Result<Self, MarkCatalogError> {
        let doc: MarksDoc = serde_json::from_str(json).map_err(|error| MarkCatalogError {
            message: format!("invalid marks.json: {error}"),
        })?;
        if doc.schema_version != 1 {
            return Err(MarkCatalogError {
                message: format!(
                    "unsupported marks schema {}; expected 1",
                    doc.schema_version
                ),
            });
        }
        for (kind, spec) in &doc.kinds {
            if spec.anchors.is_empty() {
                return Err(MarkCatalogError {
                    message: format!("mark kind {kind} accepts no anchors"),
                });
            }
            if !(spec.half_life_days_wet > 0.0 && spec.half_life_days_dry > 0.0) {
                return Err(MarkCatalogError {
                    message: format!("mark kind {kind} has a non-positive half-life"),
                });
            }
            // The two field names differ by three characters, and swapping
            // them inverts the whole model in silence: rain would *preserve*
            // chalk, and an arcade would wash it off faster than an open wall,
            // because `half_life_days` lerps dry → wet on precipitation and
            // shelter only moves you back toward the dry end.
            if spec.half_life_days_wet > spec.half_life_days_dry {
                return Err(MarkCatalogError {
                    message: format!(
                        "mark kind {kind} lasts longer wet than dry \
                         (half_life_days_wet {} > half_life_days_dry {}) — \
                         the two are swapped",
                        spec.half_life_days_wet, spec.half_life_days_dry
                    ),
                });
            }
            if spec.sheltered_multiplier < 1.0 {
                return Err(MarkCatalogError {
                    message: format!(
                        "mark kind {kind} shelters worse than open air \
                         (sheltered_multiplier {} < 1)",
                        spec.sheltered_multiplier
                    ),
                });
            }
            // `gone_below` must be strictly positive, not merely below
            // `faint_below`. At zero — the natural way to spell "never remove
            // this kind" — strength decays exponentially toward 0.0, underflows
            // to exactly 0.0, and `strength < gone_below` is then `0.0 < 0.0`,
            // which is false forever: the mark becomes immortal and permanently
            // faint, holding a slot of `MARKS_MAX` nothing can reclaim.
            if !(spec.gone_below > 0.0
                && spec.gone_below < spec.faint_below
                && spec.faint_below <= 1.0)
            {
                return Err(MarkCatalogError {
                    message: format!(
                        "mark kind {kind} needs 0 < gone_below < faint_below <= 1 \
                         (got {} and {})",
                        spec.gone_below, spec.faint_below
                    ),
                });
            }
        }
        // Every kind the Rust enum knows must be authored. A kind with no
        // catalog entry is worse than useless: `spec()` returns `None`, so the
        // sweep skips it (it never weathers, never washes off, never frees its
        // slot) while a fail-open `is_faint` would report it *binding*. A mark
        // nobody can read would go on refusing sales forever. Catching it at
        // load is the only place that costs nothing.
        for kind in [
            MarkKind::ChalkCross,
            MarkKind::WellTally,
            MarkKind::WardSign,
        ] {
            if !doc.kinds.contains_key(&kind) {
                return Err(MarkCatalogError {
                    message: format!(
                        "mark kind {kind} is missing from the catalog; every kind \
                         must be authored or its marks become immortal and unreadable"
                    ),
                });
            }
        }
        Ok(Self { kinds: doc.kinds })
    }

    pub fn spec(&self, kind: MarkKind) -> Option<&MarkKindSpec> {
        self.kinds.get(&kind)
    }

    /// Whether a kind may hang on an anchor at all — the `already_marked`
    /// check's sibling, and the reason `draw_mark` cannot put a well-tally on
    /// somebody's front door.
    pub fn accepts(&self, kind: MarkKind, anchor: &MarkAnchor) -> bool {
        self.spec(kind)
            .is_some_and(|spec| spec.anchors.contains(&anchor.slot()))
    }

    /// How a mark reads right now, qualified when it has half washed off.
    pub fn label_for(&self, mark: &Mark) -> String {
        let Some(spec) = self.spec(mark.kind) else {
            return mark.kind.as_str().to_string();
        };
        let base = if self.is_faint(mark) {
            &spec.faint_label
        } else {
            &spec.label
        };
        if mark.kind == MarkKind::WellTally {
            format!("{} ({})", base, stroke_count(mark.strokes))
        } else {
            base.clone()
        }
    }

    pub fn meaning_for(&self, mark: &Mark) -> &str {
        self.spec(mark.kind)
            .map_or("", |spec| spec.meaning.as_str())
    }

    /// Below the catalog's `faint_below`. **The one predicate every reader
    /// must consult** — a faint mark renders but does not rule.
    pub fn is_faint(&self, mark: &Mark) -> bool {
        match self.spec(mark.kind) {
            Some(spec) => mark.strength < spec.faint_below,
            None => true,
        }
    }

    /// Live *and* still ruling: what a reader means by "there is a cross here".
    ///
    /// **Fails closed.** A mark whose kind is not in the catalog binds
    /// nothing. Written the other way round — as `!is_faint` over an
    /// `is_some_and` — an unauthored kind would report *not faint* and
    /// therefore *binding*, so a mark nobody can name, label or weather would
    /// go on refusing sales forever. The catalog loader now rejects a missing
    /// kind outright, so this is the second of two locks on the same door.
    pub fn is_binding(&self, mark: &Mark) -> bool {
        self.spec(mark.kind)
            .is_some_and(|spec| mark.strength >= spec.faint_below)
    }

    /// The place of resort a ward's sign may name.
    pub fn ward_sign_place(&self, ward: &str) -> Option<&str> {
        self.spec(MarkKind::WardSign)
            .and_then(|spec| spec.places.get(ward))
            .map(String::as_str)
    }

    /// Every authored ward-sign place, ward key and all.
    pub fn ward_sign_places(&self) -> impl Iterator<Item = (&str, &str)> {
        self.spec(MarkKind::WardSign)
            .into_iter()
            .flat_map(|spec| spec.places.iter())
            .map(|(ward, place)| (ward.as_str(), place.as_str()))
    }
}

/// How a tally reads. Saturates at [`TALLY_STROKES_MAX`] rather than printing
/// a number nobody would count off a wall, and spells zero honestly: a tally
/// with no notches is one somebody started and abandoned, and claiming a draw
/// that never happened would feed the M4 reader a lie.
fn stroke_count(strokes: u32) -> String {
    match strokes {
        0 => "no strokes yet".to_string(),
        1 => "one stroke".to_string(),
        2 => "two strokes".to_string(),
        3 => "three strokes".to_string(),
        4 => "four strokes".to_string(),
        5 => "five strokes".to_string(),
        6 => "six strokes".to_string(),
        n if n >= TALLY_STROKES_MAX => format!("{TALLY_STROKES_MAX} strokes and more"),
        n => format!("{n} strokes"),
    }
}

// --------------------------------------------------------------------------- //
// Resolving an anchor
// --------------------------------------------------------------------------- //

/// Turn an anchor into a place in the world. **The only resolver** — every
/// reader, the sweep, the prompt and the render all come through here, so a
/// building that moves or a binding that changes is handled in one place.
///
/// `None` means the anchor no longer resolves. That is not an error: homes
/// rebind and places get renamed, and a dangling mark must never panic or
/// leak. The sweep drops them.
pub fn anchor_site(world: &World, anchor: &MarkAnchor) -> Option<AnchorSite> {
    match anchor {
        MarkAnchor::Household(owner) => {
            let entry = world.places.home_of(owner)?;
            Some(AnchorSite {
                point: entry.point,
                label: entry.name.clone(),
                occupant: Some(owner.clone()),
            })
        }
        MarkAnchor::Place(name) => {
            let entry = world.places.named(name)?;
            Some(AnchorSite {
                point: entry.point,
                label: entry.name.clone(),
                occupant: None,
            })
        }
    }
}

// --------------------------------------------------------------------------- //
// Weathering
// --------------------------------------------------------------------------- //

/// The half-life a mark of this kind has under this much rain, sheltered or
/// not. Pure arithmetic, kept separate from the sweep so a test can pin the
/// curve without building a world.
pub fn half_life_days(spec: &MarkKindSpec, precipitation: f64, sheltered: bool) -> f64 {
    let mut wet = precipitation.clamp(0.0, 1.0);
    if sheltered {
        wet /= spec.sheltered_multiplier;
    }
    spec.half_life_days_dry + (spec.half_life_days_wet - spec.half_life_days_dry) * wet
}

/// The quantized strength [`PublicMark`](crate::snapshot::PublicMark)
/// publishes — the whole-percent opacity step the renderer can actually see.
///
/// Shared by the sweep's change test and by `World::public_snapshot` so the
/// two cannot drift: if the sweep decided "changed" on a finer grain than the
/// wire carries, every mark in the city would churn the snapshot chain
/// forever. A non-finite strength quantizes to 0 rather than panicking.
pub fn published_strength_pct(strength: f64) -> u8 {
    if !strength.is_finite() {
        return 0;
    }
    (strength.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Weather every mark, drop the ones that have washed off or whose anchor
/// stopped resolving, and remember when we last did it.
///
/// Gated to once a game-minute: the engine polls at ~60 Hz and strength moves
/// in days. Returns whether anything changed, which is the caller's cue to
/// bump the world revision — a sweep that moved nothing must not churn the
/// snapshot chain.
pub fn sweep(world: &mut World, game_days: f64) -> bool {
    if world.marks.is_empty() {
        // Keep the clock current even with nothing to weather, so the first
        // mark drawn into a long-running world is not charged for all of it.
        world.marks.last_sweep_game_days = game_days;
        return false;
    }
    let previous = world.marks.last_sweep_game_days;
    if previous.is_finite()
        && (game_days * SWEEPS_PER_GAME_DAY).floor() <= (previous * SWEEPS_PER_GAME_DAY).floor()
    {
        return false;
    }

    // Resolve every site under one immutable borrow before touching anything:
    // `anchor_site` reads `world.places`, which we cannot hold across a
    // mutation of `world.marks`.
    let precipitation = world
        .current_weather
        .map_or(0.0, |sample| sample.precipitation);
    // A non-finite or negative scale would make `elapsed` NaN, slip past the
    // `elapsed <= 0.0` guard below (every comparison with NaN is false), and
    // poison `strength` for the rest of the run — after which `gone_below`
    // never fires and the mark is immortal. `config.ron` and
    // `--marks-decay-scale` both accept a bare `NaN`, so this is reachable
    // from outside the code.
    let decay_scale = if world.marks.decay_scale.is_finite() && world.marks.decay_scale > 0.0 {
        world.marks.decay_scale
    } else {
        1.0
    };
    let doomed: Vec<MarkId> = world
        .marks
        .iter()
        .filter(|(_, mark)| anchor_site(world, &mark.anchor).is_none())
        .map(|(id, _)| id)
        .collect();
    let sheltered: Vec<(MarkId, bool)> = world
        .marks
        .iter()
        .filter_map(|(id, mark)| {
            let site = anchor_site(world, &mark.anchor)?;
            Some((id, world.shelters.is_sheltered(site.point)))
        })
        .collect();

    let catalog = world.mark_catalog.clone();
    let mut changed = !doomed.is_empty();
    for id in doomed {
        world.marks.remove(id);
    }
    let mut washed_off = Vec::new();
    for (id, is_sheltered) in sheltered {
        let Some(mark) = world.marks.get_mut(id) else {
            continue;
        };
        let Some(spec) = catalog.spec(mark.kind) else {
            continue;
        };
        let elapsed = (game_days - mark.last_decayed_game_days).max(0.0) * decay_scale;
        mark.last_decayed_game_days = game_days;
        if elapsed <= 0.0 {
            continue;
        }
        let half = half_life_days(spec, precipitation, is_sheltered);
        let before = published_strength_pct(mark.strength);
        mark.strength *= 0.5f64.powf(elapsed / half);
        // Compare the *published* value, not the raw f64. A game-minute of a
        // nine-day half-life multiplies strength by 0.99995 — a change twelve
        // orders of magnitude above f64 epsilon — so `strength != before`
        // would be true on literally every sweep. That returns `changed` every
        // game-minute for as long as one mark exists anywhere in the city, and
        // the engine bumps `world_revision` on a true return: a full 137 KB
        // snapshot re-serialized and re-sent every 2.5 real seconds, forever,
        // because a cross was drying somewhere. The renderer can only see
        // whole percent steps, so those are what "changed" has to mean.
        if published_strength_pct(mark.strength) != before {
            changed = true;
        }
        if mark.strength < spec.gone_below {
            washed_off.push(id);
        }
    }
    for id in washed_off {
        world.marks.remove(id);
        changed = true;
    }
    world.marks.last_sweep_game_days = game_days;
    changed
}

// --------------------------------------------------------------------------- //
// Reading, for the prompt and the HUD
// --------------------------------------------------------------------------- //

/// One mark near somebody, with everything a bullet needs.
#[derive(Debug, Clone, PartialEq)]
pub struct NearbyMark {
    pub id: MarkId,
    pub kind: MarkKind,
    pub label: String,
    pub meaning: String,
    pub site_label: String,
    pub occupant: Option<ActorId>,
    pub distance_m: f64,
}

/// Every live mark whose anchor site is within `radius_m` of `point`, nearest
/// first, ties broken by id so a replay renders the same sheet.
pub fn marks_within(world: &World, point: Vec3, radius_m: f64) -> Vec<NearbyMark> {
    // Ablated (`CATHEDRAL_NO_MARKS`) this is empty: the switch has to reach
    // the *readers*, not only the writer, or an ablation run would still
    // render every mark on the walls and still refuse sales because of them —
    // which is not an ablation of anything.
    if !world.marks_enabled {
        return Vec::new();
    }
    let mut found: Vec<NearbyMark> = world
        .marks
        .iter()
        .filter_map(|(id, mark)| {
            let site = anchor_site(world, &mark.anchor)?;
            let distance_m = site.point.distance(point);
            if distance_m > radius_m {
                return None;
            }
            Some(NearbyMark {
                id,
                kind: mark.kind,
                label: world.mark_catalog.label_for(mark),
                meaning: world.mark_catalog.meaning_for(mark).to_string(),
                site_label: site.label,
                occupant: site.occupant,
                distance_m,
            })
        })
        .collect();
    found.sort_by(|left, right| {
        left.distance_m
            .partial_cmp(&right.distance_m)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    found
}

/// Whether a live, still-*binding* mark of `kind` is about `who`. The shape
/// every rule-reader uses, and the reason a half-washed cross stops refusing
/// anybody.
///
/// Reads `world.marks` and nothing else — in particular never
/// `world.notices`, which is what makes scrubbing real rather than cosmetic.
pub fn binding_mark_about(world: &World, kind: MarkKind, who: &ActorId) -> Option<MarkId> {
    if !world.mark_kind_enabled(kind) {
        return None;
    }
    world
        .marks
        .iter()
        .find(|(_, mark)| {
            mark.kind == kind
                && mark.about.as_ref() == Some(who)
                && world.mark_catalog.is_binding(mark)
        })
        .map(|(id, _)| id)
}

/// Draw, or refresh what is already there. The shape every writer uses:
/// idempotent by (kind, anchor), so a beat that runs every day leaves one
/// mark and not a wall of them.
///
/// `author` is `None` for the ward's own hand.
pub fn draw_or_refresh(
    world: &mut World,
    kind: MarkKind,
    anchor: MarkAnchor,
    author: Option<ActorId>,
    game_days: f64,
) -> Option<Drawn> {
    if !world.mark_kind_enabled(kind) || !world.mark_catalog.accepts(kind, &anchor) {
        return None;
    }
    let site = anchor_site(world, &anchor)?;
    if let Some((id, _)) = world.marks.find(kind, &anchor) {
        if let Some(mark) = world.marks.get_mut(id) {
            mark.strength = 1.0;
            mark.last_decayed_game_days = game_days;
        }
        return Some(Drawn {
            id,
            fresh: false,
            evicted: None,
        });
    }
    let mark = Mark {
        kind,
        about: site.occupant,
        anchor,
        author,
        drawn_game_days: game_days,
        last_decayed_game_days: game_days,
        strength: 1.0,
        strokes: 1,
    };
    let (id, evicted) = world.marks.insert(mark);
    Some(Drawn {
        id,
        fresh: true,
        evicted,
    })
}

/// What a draw did. `evicted` is §2.7's cap in action: at [`MARKS_MAX`] the
/// faintest mark makes way, and the caller is expected to say so — a mark
/// vanishing from the world with no diagnostic anywhere is exactly the kind of
/// silent state loss the cap exists to make legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drawn {
    pub id: MarkId,
    /// `false` when an existing mark was refreshed rather than a new one made.
    pub fresh: bool,
    /// The mark the cap pushed off the walls to make room, if any.
    pub evicted: Option<MarkId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> MarkKindSpec {
        MarkCatalog::default()
            .spec(MarkKind::ChalkCross)
            .expect("the embedded catalog has a chalk cross")
            .clone()
    }

    #[test]
    fn the_embedded_catalog_parses_and_has_every_kind() {
        let catalog = MarkCatalog::from_embedded().expect("embedded catalog parses");
        for kind in [
            MarkKind::ChalkCross,
            MarkKind::WellTally,
            MarkKind::WardSign,
        ] {
            let spec = catalog.spec(kind).expect("every kind is authored");
            assert!(!spec.label.is_empty(), "{kind} has no label");
            assert!(!spec.meaning.is_empty(), "{kind} has no meaning");
        }
    }

    #[test]
    fn a_cross_hangs_on_a_household_and_a_tally_does_not() {
        let catalog = MarkCatalog::default();
        let household = MarkAnchor::Household(ActorId::from_raw("p001v"));
        let place = MarkAnchor::Place("Chain Well".to_string());
        assert!(catalog.accepts(MarkKind::ChalkCross, &household));
        assert!(!catalog.accepts(MarkKind::ChalkCross, &place));
        assert!(catalog.accepts(MarkKind::WellTally, &place));
        assert!(!catalog.accepts(MarkKind::WellTally, &household));
    }

    #[test]
    fn the_dry_half_life_is_the_authored_one() {
        // Deliberately NOT `0.5f64.powf(half / half) == 0.5`: that is an
        // identity about `f64::powf` for any half-life whatsoever, reads like
        // a decay test, and passes with the entire weathering deleted. The
        // exponential itself is pinned end-to-end against a real `World` in
        // `tests/marks_tests.rs::chalk_halves_over_one_dry_half_life`; what is
        // worth asserting *here* is that dry air uses the dry number.
        let spec = spec();
        assert_eq!(half_life_days(&spec, 0.0, false), spec.half_life_days_dry);
        // The wet end is a lerp, so it lands within a float epsilon rather
        // than exactly on the authored number.
        assert!(
            (half_life_days(&spec, 1.0, false) - spec.half_life_days_wet).abs() < 1e-9,
            "full rain should reach the authored wet half-life"
        );
    }

    #[test]
    fn the_published_strength_survives_a_poisoned_one() {
        assert_eq!(published_strength_pct(1.0), 100);
        assert_eq!(published_strength_pct(0.0), 0);
        assert_eq!(published_strength_pct(0.335), 34);
        assert_eq!(published_strength_pct(f64::NAN), 0);
        assert_eq!(published_strength_pct(f64::INFINITY), 0);
        assert_eq!(published_strength_pct(-5.0), 0);
        assert_eq!(published_strength_pct(5.0), 100);
    }

    #[test]
    fn a_catalog_missing_a_kind_is_refused() {
        let json = r#"{
          "schema_version": 1,
          "kinds": {
            "chalk_cross": {
              "label": "x", "meaning": "y", "faint_label": "z",
              "anchors": ["household"],
              "half_life_days_dry": 9.0, "half_life_days_wet": 0.4,
              "sheltered_multiplier": 6.0,
              "faint_below": 0.35, "gone_below": 0.05,
              "drawable_by_hand": true
            }
          }
        }"#;
        let error = MarkCatalog::from_json(json).expect_err("a partial catalog is refused");
        assert!(
            error.message.contains("missing from the catalog"),
            "unhelpful message: {}",
            error.message
        );
    }

    #[test]
    fn an_unauthored_kind_binds_nothing() {
        // The second lock. `is_binding` must not be `!is_faint` over an
        // `is_some_and`, or a mark nobody can name would refuse sales forever.
        let orphan = Mark {
            kind: MarkKind::WardSign,
            anchor: MarkAnchor::Place("anywhere".into()),
            about: None,
            author: None,
            drawn_game_days: 0.0,
            last_decayed_game_days: 0.0,
            strength: 1.0,
            strokes: 1,
        };
        assert!(
            MarkCatalog::default().is_binding(&orphan),
            "sanity: an authored kind at full strength does bind"
        );
        let partial = MarkCatalog {
            kinds: BTreeMap::new(),
        };
        assert!(
            !partial.is_binding(&orphan),
            "an unauthored kind binds nothing"
        );
        assert!(partial.is_faint(&orphan), "and reads as faint, not fresh");
    }

    #[test]
    fn a_swapped_half_life_pair_is_refused() {
        let json = r#"{
          "schema_version": 1,
          "kinds": {
            "chalk_cross": {
              "label": "x", "meaning": "y", "faint_label": "z",
              "anchors": ["household"],
              "half_life_days_dry": 9.0, "half_life_days_wet": 90.0,
              "sheltered_multiplier": 6.0,
              "faint_below": 0.35, "gone_below": 0.05,
              "drawable_by_hand": true
            }
          }
        }"#;
        let error = MarkCatalog::from_json(json).expect_err("a swapped pair is refused");
        assert!(
            error.message.contains("lasts longer wet than dry"),
            "unhelpful message: {}",
            error.message
        );
    }

    #[test]
    fn a_zero_gone_below_is_refused() {
        let json = r#"{
          "schema_version": 1,
          "kinds": {
            "chalk_cross": {
              "label": "x", "meaning": "y", "faint_label": "z",
              "anchors": ["household"],
              "half_life_days_dry": 9.0, "half_life_days_wet": 0.4,
              "sheltered_multiplier": 6.0,
              "faint_below": 0.35, "gone_below": 0.0,
              "drawable_by_hand": true
            }
          }
        }"#;
        let error = MarkCatalog::from_json(json).expect_err("gone_below 0 is refused");
        assert!(
            error.message.contains("0 < gone_below"),
            "unhelpful message: {}",
            error.message
        );
    }

    #[test]
    fn a_tally_never_claims_a_draw_that_did_not_happen() {
        assert_eq!(stroke_count(0), "no strokes yet");
        assert_eq!(stroke_count(1), "one stroke");
        assert_eq!(
            stroke_count(TALLY_STROKES_MAX),
            format!("{TALLY_STROKES_MAX} strokes and more")
        );
        assert_eq!(
            stroke_count(u32::MAX),
            format!("{TALLY_STROKES_MAX} strokes and more"),
            "a wall never reads out a 10-digit number"
        );
    }

    #[test]
    fn rain_shortens_the_half_life_and_shelter_gives_it_back() {
        let spec = spec();
        let dry = half_life_days(&spec, 0.0, false);
        let wet = half_life_days(&spec, 1.0, false);
        let sheltered = half_life_days(&spec, 1.0, true);
        assert!(wet < dry, "rain must wash chalk faster: {wet} vs {dry}");
        assert!(
            sheltered > wet,
            "a sheltered wall must outlast an open one under the same rain: {sheltered} vs {wet}"
        );
        assert!(
            sheltered < dry,
            "shelter is cover, not a roof sealed against weather: {sheltered} vs {dry}"
        );
    }

    #[test]
    fn eviction_at_the_cap_takes_the_faintest_and_is_deterministic() {
        let mut marks = Marks::default();
        for index in 0..MARKS_MAX {
            let mark = Mark {
                kind: MarkKind::ChalkCross,
                anchor: MarkAnchor::Household(ActorId::from_raw(format!("p{index:04}"))),
                about: None,
                author: None,
                drawn_game_days: 0.0,
                last_decayed_game_days: 0.0,
                // Descending, so the faintest is the LAST inserted — proving
                // eviction picks by strength and not by age.
                strength: 1.0 - (index as f64) * 0.001,
                strokes: 1,
            };
            let (_, evicted) = marks.insert(mark);
            assert!(evicted.is_none(), "no eviction below the cap");
        }
        assert_eq!(marks.len(), MARKS_MAX);
        let faintest = marks
            .iter()
            .min_by(|(_, left), (_, right)| left.strength.total_cmp(&right.strength))
            .map(|(id, _)| id)
            .expect("a faintest mark exists");

        let overflow = Mark {
            kind: MarkKind::ChalkCross,
            anchor: MarkAnchor::Household(ActorId::from_raw("straw")),
            about: None,
            author: None,
            drawn_game_days: 1.0,
            last_decayed_game_days: 1.0,
            strength: 1.0,
            strokes: 1,
        };
        let (fresh, evicted) = marks.insert(overflow);
        assert_eq!(
            evicted,
            Some(faintest),
            "the faintest mark is the one that goes"
        );
        assert_eq!(marks.len(), MARKS_MAX, "the cap holds");
        assert!(marks.get(fresh).is_some(), "the new mark is in");
        assert!(marks.get(faintest).is_none(), "the evicted mark is out");
    }

    #[test]
    fn ids_are_never_reused_after_an_eviction() {
        let mut marks = Marks::default();
        let make = || Mark {
            kind: MarkKind::ChalkCross,
            anchor: MarkAnchor::Household(ActorId::from_raw("p001v")),
            about: None,
            author: None,
            drawn_game_days: 0.0,
            last_decayed_game_days: 0.0,
            strength: 1.0,
            strokes: 1,
        };
        let (first, _) = marks.insert(make());
        marks.remove(first);
        let (second, _) = marks.insert(make());
        assert_ne!(first, second, "a removed id must never come back");
    }

    #[test]
    fn every_authored_ward_sign_place_is_spelled_for_a_real_ward() {
        let catalog = MarkCatalog::default();
        let wards: Vec<&str> = catalog.ward_sign_places().map(|(ward, _)| ward).collect();
        assert_eq!(
            wards.len(),
            8,
            "one place of resort per ward, got {wards:?}"
        );
        for ward in [
            "fabric",
            "wick",
            "cloth",
            "wallwright",
            "cinder",
            "weigh",
            "reed",
            "bell_and_sluice",
        ] {
            assert!(
                catalog.ward_sign_place(ward).is_some(),
                "ward {ward} has no place of resort to chalk"
            );
        }
    }

    #[test]
    fn a_catalog_that_shelters_worse_than_open_air_is_refused() {
        let json = r#"{
          "schema_version": 1,
          "kinds": {
            "chalk_cross": {
              "label": "x", "meaning": "y", "faint_label": "z",
              "anchors": ["household"],
              "half_life_days_dry": 9.0, "half_life_days_wet": 0.4,
              "sheltered_multiplier": 0.5,
              "faint_below": 0.35, "gone_below": 0.05,
              "drawable_by_hand": true
            }
          }
        }"#;
        let error = MarkCatalog::from_json(json).expect_err("a multiplier below 1 is refused");
        assert!(
            error.message.contains("shelters worse than open air"),
            "unhelpful message: {}",
            error.message
        );
    }

    #[test]
    fn an_unknown_catalog_field_is_refused() {
        let json = r#"{
          "schema_version": 1,
          "kinds": {
            "chalk_cross": {
              "label": "x", "meaning": "y", "faint_label": "z",
              "anchors": ["household"],
              "half_life_days_dry": 9.0, "half_life_days_wet": 0.4,
              "sheltered_multiplier": 6.0,
              "faint_below": 0.35, "gone_below": 0.05,
              "drawable_by_hand": true,
              "half_life_days_damp": 2.0
            }
          }
        }"#;
        assert!(
            MarkCatalog::from_json(json).is_err(),
            "deny_unknown_fields must catch a misspelled dial"
        );
    }
}
