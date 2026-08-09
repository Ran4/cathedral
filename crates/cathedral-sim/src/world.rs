//! The world (`sim.py:159-389`): entities, the event buffer, spatial updates,
//! the public snapshot, and the invariants a sim bug would break.
//!
//! Maps are `BTreeMap` (deterministic iteration; Rust `String` Ord is UTF-8
//! byte order, which equals Python's code-point order). Round-robin scheduling
//! needs *insertion* order instead, so [`World::roster`] records it (D12).

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{
    AVOID_MAX_NEIGHBOURS, AVOID_PERSONAL_RADIUS_M, AVOID_PUSH_MPS, DEFAULT_VIEW_CONE_DEGREES,
    LANE_JITTER_FRACTION, LANE_KEEP_RIGHT_FRACTION, NEEDLE_CLAIM_RADIUS_M, NEEDLE_REROUTE_SECONDS,
    WALK_SPEED_MPS,
    areas::AreaMap,
    attention::DEFAULT_STAGE_RADIUS_M,
    character::{Character, Control, Presence, StatusKind},
    clock::WorldTime,
    error::{SpatialUpdateError, SpatialUpdateErrorCode},
    event::DomainEvent,
    ids::{ActorId, ItemId},
    inventory::{CompletedTransform, LegacyRestockShare, TransformJob},
    item::{Item, ItemCatalog},
    math::Vec3,
    nav::{NavData, WALK_Y},
    offer::Offer,
    places::PlaceRegistry,
    snapshot::{ActorSnapshot, ItemSnapshot, OfferSnapshot, PublicSnapshot},
    sounds::SoundCatalog,
    weather::{Shelter, ShelterMap, WeatherSample},
};

/// Walk-cadence multiplier: `gait_phase += speed * dt * GAIT_CADENCE`. One
/// full stride cycle (two steps) per ~1.5 m, i.e. ~2.4 steps/s at the brisk
/// 1.8 m/s — matched to the host puppet's ~±27° thigh swing so feet don't
/// visibly skate (npc_bodies M1; the phase stays the sim's single source of
/// stride truth). It never affects where anyone ends up.
const GAIT_CADENCE: f64 = 0.67;

/// One actor's new position (and optionally facing) in a spatial update.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialActorUpdate {
    pub actor_id: ActorId,
    pub position_m: Vec3,
    pub facing_yaw: Option<f64>,
}

impl SpatialActorUpdate {
    pub fn new(actor_id: ActorId, position_m: Vec3, facing_yaw: Option<f64>) -> Self {
        Self {
            actor_id,
            position_m,
            facing_yaw,
        }
    }
}

/// The Needle's one-person claim (M7): the 1.2 m alley fits one walker, so the
/// choke circle around its node is entered only by the claim's holder — or
/// behind them, going the same way. An opposing walker stands at the mouth
/// until the holder leaves (or takes the long way round after
/// [`crate::NEEDLE_REROUTE_SECONDS`]). Released the moment the holder stops
/// walking or leaves the circle, so a conversation inside the alley can never
/// deadlock the city (`features/implemented/movement/02_navigation.md` §5).
#[derive(Debug, Clone, PartialEq)]
pub struct NeedleClaim {
    pub holder: ActorId,
    /// The holder's XZ travel direction at entry; an entrant whose direction
    /// opposes it (negative dot) waits.
    pub dir: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    /// Authoritative named geography used whenever a prompt is rendered.
    pub area_map: AreaMap,
    pub characters: BTreeMap<ActorId, Character>,
    /// Character insertion order — the round-robin turn order (D12).
    pub roster: Vec<ActorId>,
    pub items: BTreeMap<ItemId, Item>,
    /// The embedded item catalog: the single source of truth for derived display
    /// names, visuals, stackability, edibility and prices. Host-provided context
    /// like `area_map` — defaulted to the crate's embedded catalog, so every
    /// `World` has it with no wiring. Shared behind an `Arc`; it never changes at
    /// runtime and never bumps `world_revision`.
    pub item_catalog: Arc<ItemCatalog>,
    /// Keyed by item id: at most one live offer per item.
    pub offers: BTreeMap<ItemId, Offer>,
    /// Operational M5 inventory state. Neither map is part of the public
    /// snapshot; both are nevertheless authoritative and clone with the world.
    pub(crate) legacy_restock_shares: BTreeMap<ItemId, Vec<LegacyRestockShare>>,
    pub(crate) transform_jobs: BTreeMap<ActorId, TransformJob>,
    pub(crate) completed_transform_jobs: BTreeMap<String, CompletedTransform>,
    /// Monotonic public-state counter.
    pub world_revision: i64,
    /// Last issued `DomainEvent` sequence.
    pub event_sequence: i64,
    /// Last applied spatial update sequence; starts at -1 so seq 0 is accepted.
    pub spatial_sequence: i64,
    pub sounds_enabled: bool,
    pub view_cone_degrees: f64,
    /// The rows `make_sound` and the world-sound triggers resolve against.
    pub sound_catalog: SoundCatalog,
    /// The resolved world time, refreshed by the engine each poll and read by the
    /// prompt renderer for `you_are.the_hour`. Host-provided context like
    /// `area_map`; it never bumps `world_revision` (the office changes far too
    /// rarely to be worth a snapshot, and the clock advances every frame). `None`
    /// until a clock-bearing host sets it — so a prompt rendered without one (the
    /// frozen golden fixtures) simply omits the hour.
    pub current_time: Option<WorldTime>,
    /// Authoritative hot weather context. Like `current_time`, this is sampled
    /// by the engine before each deterministic round and never changes the
    /// cold public snapshot revision.
    pub current_weather: Option<WeatherSample>,
    /// Data-owned social shelter destinations. Presentation has a denser cover
    /// map, while this map names only places an actor can credibly occupy.
    pub shelters: Arc<ShelterMap>,
    /// The street graph, when the host carries one — host-provided context like
    /// `area_map`, set by the engine at construction. `go_to` validates and
    /// prices its route against it at intent time; `None` (the default) makes
    /// every `go_to` fail `no_route`, which is honest for a world nobody can
    /// walk in.
    pub nav: Option<Arc<NavData>>,
    /// The wayfinding registry `places_you_know` renders from and `go_to` /
    /// `tell_way` resolve against. Empty (the default) in a world without a
    /// nav graph.
    pub places: PlaceRegistry,
    /// Who currently holds the Needle's one-person choke (M7), or `None` while
    /// it stands empty. Movement state like a mover's own path: it rides the
    /// fixed tick and never touches the public revision.
    pub needle_claim: Option<NeedleClaim>,
    /// The ward's live notices (`law_and_order.md` M3). Rendered on carriers'
    /// sheets and settled by `accept_offered_item`, so world state — but,
    /// like `current_time`, never part of the public snapshot and never a
    /// `world_revision` bump: the player is meant to feel the city cooling,
    /// not read a wanted list.
    pub notices: crate::notices::Notices,
    /// Everyone the law is holding (`law_and_order.md` M4/M5) — in charge on an
    /// escort, or committed at a station. World state for the same reasons the
    /// notices are: the prompt renders it, the behaviour ladder is guarded by it
    /// and `go_to` is refused through it. Custody *is* published to the host,
    /// but on its own hot channel ([`crate::engine::EngineMessage::LawStanding`])
    /// rather than through the cold snapshot, because the tether it drives has
    /// to be exact at 20 Hz.
    pub custody: crate::custody::Custody,
    /// The street dogs ([`crate::dogs`]) — sim-owned wanderers with a position
    /// and nothing else of a character: no sheet, no inbox, no turns, never in
    /// `characters_within`. Every nearby sheet renders them under
    /// `**dogs_nearby**`; their poses ride their own hot channel
    /// ([`crate::engine::EngineMessage::Dogs`]) and, like a mover's path,
    /// never touch the public revision. Empty — the default, and the frozen
    /// fixtures' case — until the engine seeds the authored pack.
    pub dogs: Vec<crate::dogs::Dog>,
    /// The chalk on the walls ([`crate::marks`]) — the one piece of world
    /// state the player can rewrite with a wet sleeve.
    ///
    /// Unlike [`notices`](Self::notices), marks *are* published: they are what
    /// the render draws. But unlike the dogs' poses they are slow, so they
    /// ride the cold public snapshot and bump the revision rather than taking
    /// a hot channel. Empty — the default, and every frozen fixture's case —
    /// until a hand chalks something.
    pub marks: crate::marks::Marks,
    /// What each kind of mark means and how it weathers, authored in
    /// `assets/world/marks.json` and embedded at build time. An `Arc` because
    /// the decay sweep needs it while holding `marks` mutably.
    pub mark_catalog: Arc<crate::marks::MarkCatalog>,
    /// Whether hands may chalk at all (`config.ron:
    /// smart_actors.marks.enabled`, `CATHEDRAL_NO_MARKS`). Ablation only —
    /// turning it off stops new marks and stops the readers; it does not erase
    /// what is already on the walls.
    pub marks_enabled: bool,
    /// The per-kind switches (`config.ron: smart_actors.marks.cross` / `.tally`
    /// / `.ward_sign`), so one writer can be silenced without losing the
    /// medium — which is how you tell whether the well tally or the cross is
    /// responsible for something you are watching.
    pub mark_kinds: crate::marks::MarkKindSwitches,
    /// What each ward is saying to itself tonight (movement M6): the Night
    /// Office's ward batch returns a few sentences of mood, and every Minor of
    /// that ward carries it on their sheet until the next night rewrites it.
    ///
    /// It is the Minors' whole share of reflection — one prompt buys a hundred
    /// and twenty people a changed outlook — so it is world state like
    /// [`notices`](Self::notices), rendered by the prompt and never part of the
    /// public snapshot: the player is meant to hear the mood in what people
    /// say, not read it off a panel.
    pub ward_moods: BTreeMap<crate::lore::PlanningWard, String>,
    /// Who has spoken so far in the reply currently being applied, cleared by
    /// the scheduler before each one.
    ///
    /// Exactly one verb reads it: `seize` (`law_and_order.md` M4), which may not
    /// be wordless — a seizure with no `say` in the same turn reads as the game
    /// stealing the controller. Everywhere else the same rule is prompt
    /// guidance (turn.j2's "setting off in silence looks to them like being
    /// ignored"); here it is worth enforcing, because this is the one verb that
    /// takes the player's feet.
    pub(crate) spoke_this_turn: Option<ActorId>,
    events: Vec<DomainEvent>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            area_map: AreaMap::default(),
            characters: BTreeMap::new(),
            roster: Vec::new(),
            items: BTreeMap::new(),
            item_catalog: ItemCatalog::embedded(),
            offers: BTreeMap::new(),
            legacy_restock_shares: BTreeMap::new(),
            transform_jobs: BTreeMap::new(),
            completed_transform_jobs: BTreeMap::new(),
            world_revision: 0,
            event_sequence: 0,
            spatial_sequence: -1,
            sounds_enabled: true,
            view_cone_degrees: DEFAULT_VIEW_CONE_DEGREES,
            sound_catalog: SoundCatalog::empty(),
            current_time: None,
            current_weather: None,
            shelters: Arc::new(ShelterMap::default()),
            nav: None,
            places: PlaceRegistry::default(),
            needle_claim: None,
            notices: crate::notices::Notices::default(),
            custody: crate::custody::Custody::default(),
            spoke_this_turn: None,
            dogs: Vec::new(),
            marks: crate::marks::Marks::default(),
            mark_catalog: Arc::new(crate::marks::MarkCatalog::default()),
            marks_enabled: true,
            mark_kinds: crate::marks::MarkKindSwitches::default(),
            ward_moods: BTreeMap::new(),
            events: Vec::new(),
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shelter_at(&self, position: Vec3) -> Option<&Shelter> {
        self.shelters.at(position)
    }

    pub fn is_sheltered(&self, position: Vec3) -> bool {
        self.shelters.is_sheltered(position)
    }

    /// Seed-time only: a duplicate id is a programmer error, not an
    /// `ActionError`. Bumps the public revision, like Python's `add`.
    pub fn add_character(&mut self, character: Character) {
        let id = character.id().clone();
        assert!(
            !self.characters.contains_key(&id),
            "duplicate character id '{id}'"
        );
        self.roster.push(id.clone());
        self.characters.insert(id, character);
        self.touch_public_state();
    }

    /// Seed-time only: a duplicate id is a programmer error.
    pub fn add_item(&mut self, item: Item) {
        let id = item.id.clone();
        assert!(!self.items.contains_key(&id), "duplicate item id '{id}'");
        self.items.insert(id, item);
        self.touch_public_state();
    }

    /// Whether this kind of chalk is live at all — the whole-layer switch and
    /// the per-kind one together. Every writer and every reader in
    /// [`crate::marks`] goes through this, so an ablation run is a real
    /// ablation rather than one that merely stops new marks appearing.
    pub fn mark_kind_enabled(&self, kind: crate::marks::MarkKind) -> bool {
        self.marks_enabled && self.mark_kinds.enabled(kind)
    }

    pub fn touch_public_state(&mut self) -> i64 {
        self.world_revision += 1;
        self.world_revision
    }

    /// The one physical-presence predicate used at every world-facing seam.
    pub fn is_present(&self, actor_id: &ActorId) -> bool {
        self.characters
            .get(actor_id)
            .is_some_and(|actor| actor.state.presence == Presence::InCity)
    }

    /// Whoever this world's [`Control::Player`] is, when it has one. Resolved
    /// from control rather than from the engine's configured id, so the sim
    /// layer can answer "is this the one body I do not own the feet of?"
    /// without being handed the answer ([`crate::custody`]).
    pub fn player_id(&self) -> Option<&ActorId> {
        self.characters
            .iter()
            .find(|(_, character)| character.control() == Control::Player)
            .map(|(id, _)| id)
    }

    pub fn presence_epoch(&self, actor_id: &ActorId) -> Option<u64> {
        self.characters
            .get(actor_id)
            .map(|actor| actor.state.presence_epoch)
    }

    /// Atomically enter or remove a group. Epoch arithmetic is preflighted for
    /// every member; the public revision advances exactly once for the batch.
    /// On departure city-transient actor state is discarded, while memories,
    /// goals, relationships and `recent_history` remain untouched.
    pub fn transition_presence(
        &mut self,
        members: &[ActorId],
        presence: Presence,
        entry_positions: &BTreeMap<ActorId, Vec3>,
    ) -> Result<Vec<(ActorId, u64)>, String> {
        if members.is_empty() {
            return Err("a presence transition needs at least one member".to_string());
        }
        let mut next = Vec::with_capacity(members.len());
        let mut seen = std::collections::BTreeSet::new();
        for member in members {
            if !seen.insert(member) {
                return Err(format!("duplicate party member '{member}'"));
            }
            let actor = self
                .characters
                .get(member)
                .ok_or_else(|| format!("unknown party member '{member}'"))?;
            if actor.state.presence == presence {
                return Err(format!("party member '{member}' is already {presence:?}"));
            }
            let epoch = actor
                .state
                .presence_epoch
                .checked_add(1)
                .ok_or_else(|| format!("presence epoch overflow for '{member}'"))?;
            if presence == Presence::InCity && !entry_positions.contains_key(member) {
                return Err(format!("entry position missing for '{member}'"));
            }
            next.push((member.clone(), epoch));
        }
        for (member, epoch) in &next {
            let actor = self.characters.get_mut(member).expect("preflighted member");
            actor.state.presence = presence;
            actor.state.presence_epoch = *epoch;
            if presence == Presence::InCity {
                actor.state.position_m = entry_positions[member];
            } else {
                actor.state.inbox.clear();
                actor.state.pending_history.clear();
                actor.state.movement = None;
                actor.state.intent = None;
                actor.state.active_gesture = None;
                actor.state.you_sell.clear();
            }
        }
        if presence == Presence::BeyondTheWalls {
            self.offers.retain(|_, offer| {
                !seen.contains(&offer.giver_id)
                    && offer
                        .target_id
                        .as_ref()
                        .is_none_or(|target| !seen.contains(target))
            });
        }
        self.touch_public_state();
        Ok(next)
    }

    /// Debug-only carriage write (`features/npc_bodies.md` §8): set `kind` to a
    /// clamped `0..=1` `value` on the character whose display name matches
    /// `name` (case-insensitive). The **sole** non-test writer of `statuses` —
    /// the `cathedral-headless --status` flag and the drive-mode `status`
    /// action both land here — so no reflex ever fabricates one; it stays a
    /// fact the sim owns. Bumps `world_revision` so the next snapshot carries
    /// it. `who` is resolved against the display **name** first
    /// (case-insensitive), then the actor **id** — the `id p006v` the HUD shows
    /// for strangers, also tolerating a pasted `p006v_ilse` lore stem by its id
    /// prefix. Returns `false` (and writes nothing) when nobody matches.
    /// Resolve a developer-supplied handle to a character.
    ///
    /// Debug tooling identifies a target by name or id (the sim proper only ever
    /// resolves ids). Name wins globally, then an exact id, then the id prefix of
    /// an `<id>_<name>` stem someone may have pasted off a prompt filename.
    pub fn resolve_debug_handle(&self, who: &str) -> Option<ActorId> {
        let id_stem = who.split('_').next().unwrap_or(who);
        self.characters
            .values()
            .find(|character| character.name().eq_ignore_ascii_case(who))
            .or_else(|| {
                self.characters.values().find(|character| {
                    character.id().as_str().eq_ignore_ascii_case(who)
                        || character.id().as_str().eq_ignore_ascii_case(id_stem)
                })
            })
            .map(|character| character.id().clone())
    }

    pub fn debug_set_status(&mut self, who: &str, kind: StatusKind, value: f64) -> bool {
        let clamped = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let Some(id) = self.resolve_debug_handle(who) else {
            return false;
        };
        if let Some(character) = self.characters.get_mut(&id) {
            character.state.statuses.insert(kind, clamped);
            // `urgency` is the one kind the sim writes for itself (the poop
            // clock, `extra_pockets.md` M3), and it rewrites the key every poll
            // — so a poke at it is *also* recorded as an override the clock
            // honours, or it would not survive to the end of this very poll.
            // Forcing `0` is how a developer gives the key back: there is
            // nothing to eyeball at zero, and the clock's own reading takes
            // over again on the next poll.
            if kind == StatusKind::Urgency {
                character.state.debug_urgency = (clamped > 0.0).then_some(clamped);
            }
            self.touch_public_state();
            true
        } else {
            false
        }
    }

    /// Characters in inclusive range of `origin`, ordered by distance then id.
    ///
    /// The order is load-bearing: it fixes `recipient_ids` in every event and
    /// the "nearest witness" the scheduler nudges. Player-controlled characters
    /// are included. A character exactly at the origin is included unless
    /// excluded.
    pub fn characters_within(
        &self,
        origin: Vec3,
        radius: f64,
        exclude: Option<&ActorId>,
    ) -> Vec<ActorId> {
        self.neighbours_by_distance(origin, radius, exclude)
            .into_iter()
            .map(|(_, id)| id.clone())
            .collect()
    }

    /// [`characters_within`] without the clones: the same set, the same order,
    /// borrowed from the world.
    ///
    /// An `ActorId` is a `String`, so the owning form costs one heap allocation
    /// per neighbour — and the two callers that pay it most (the novelty gate's
    /// [`crate::attention::context_hash`], run for every actor on stage every
    /// poll, and [`crate::attention::on_stage`], which keeps at most
    /// `max_actors` of what it scans) only ever *test* the ids they get back.
    /// Both forms are one `map` over the same scan below, so they can never
    /// drift apart and neither pays for the other's shape.
    ///
    /// [`characters_within`]: World::characters_within
    pub fn characters_within_refs(
        &self,
        origin: Vec3,
        radius: f64,
        exclude: Option<&ActorId>,
    ) -> Vec<&ActorId> {
        self.neighbours_by_distance(origin, radius, exclude)
            .into_iter()
            .map(|(_, id)| id)
            .collect()
    }

    /// The one scan and the one comparator behind both public forms: everyone
    /// in inclusive range, paired with their squared distance, ordered by
    /// distance then id.
    fn neighbours_by_distance(
        &self,
        origin: Vec3,
        radius: f64,
        exclude: Option<&ActorId>,
    ) -> Vec<(f64, &ActorId)> {
        assert!(
            radius.is_finite() && radius >= 0.0,
            "radius must be a finite non-negative number"
        );
        let radius_squared = radius * radius;
        let mut matches: Vec<(f64, &ActorId)> = self
            .characters
            .values()
            .filter(|character| character.state.presence == Presence::InCity)
            .filter(|character| Some(character.id()) != exclude)
            .filter_map(|character| {
                let distance_squared = origin.distance_squared(character.position_m());
                (distance_squared <= radius_squared).then_some((distance_squared, character.id()))
            })
            .collect();
        // No NaN can be stored, so the partial order is total here.
        matches.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .expect("positions are finite")
                .then_with(|| left.1.cmp(right.1))
        });
        matches
    }

    /// Assign the next sequence to `event`, buffer it, and return the sequence.
    pub fn emit(&mut self, mut event: DomainEvent) -> i64 {
        self.event_sequence += 1;
        event.sequence = self.event_sequence;
        self.events.push(event);
        self.event_sequence
    }

    /// Hand the buffered events to the host. Nothing is ever dropped or
    /// filtered by the sim.
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.events)
    }

    /// Validate and atomically apply a spatial update.
    ///
    /// Equal sequences are accepted only as an idempotent repeat; an equal
    /// sequence with different coordinates is rejected like an older one.
    /// Facing changes apply silently and never bump the public revision (the
    /// next snapshot always reads current facing). A **player** position change
    /// is applied but also does not bump the revision — the host owns the
    /// player's transform and the cold snapshot would only echo it back — so a
    /// public republish is owed only when a *non-player* actor is moved here.
    /// Returns whether any position changed (of any actor).
    pub fn update_positions(
        &mut self,
        spatial_sequence: i64,
        updates: &[SpatialActorUpdate],
    ) -> Result<bool, SpatialUpdateError> {
        if spatial_sequence < 0 {
            return Err(SpatialUpdateError::invalid(
                "spatial_seq must be a non-negative integer",
            ));
        }
        if spatial_sequence < self.spatial_sequence {
            return Err(SpatialUpdateError::new(
                SpatialUpdateErrorCode::StaleSpatialSeq,
                "spatial update is older than current state",
            ));
        }

        // Validate every update before mutating anything (atomicity).
        let mut seen: Vec<&ActorId> = Vec::with_capacity(updates.len());
        for update in updates {
            let actor_id = &update.actor_id;
            // Typed ids make a malformed id unreachable in practice; Python
            // raised ActionError(invalid_arguments) here, we stay in the
            // spatial error family (D16).
            if !actor_id.is_valid() {
                return Err(SpatialUpdateError::invalid(
                    "actor_id must be a non-empty character id",
                ));
            }
            if seen.contains(&actor_id) {
                return Err(SpatialUpdateError::invalid(format!(
                    "duplicate actor_id '{actor_id}' in spatial update"
                )));
            }
            seen.push(actor_id);
            if !self.characters.contains_key(actor_id) {
                return Err(SpatialUpdateError::new(
                    SpatialUpdateErrorCode::UnknownActor,
                    format!("unknown actor id '{actor_id}'"),
                ));
            }
            if !self.is_present(actor_id) {
                return Err(SpatialUpdateError::new(
                    SpatialUpdateErrorCode::UnknownActor,
                    format!("actor id '{actor_id}' is beyond the walls"),
                ));
            }
            if !update.position_m.is_finite() {
                return Err(SpatialUpdateError::invalid(
                    "position_m must be a valid finite Vec3",
                ));
            }
            if update.facing_yaw.is_some_and(|yaw| !yaw.is_finite()) {
                return Err(SpatialUpdateError::invalid(
                    "facing_yaw must be a finite number",
                ));
            }
        }

        let changed = updates
            .iter()
            .any(|update| self.characters[&update.actor_id].position_m() != update.position_m);
        // A player-only move must not republish the cold snapshot. The host owns
        // the player's own transform, so bumping the revision — and rebuilding
        // the whole `PublicSnapshot`, every actor and item — at the 10 Hz the
        // game sends spatial updates while walking is pure waste. NPC positions
        // ride the hot Movement channel (`step_movement` never touches the cold
        // state); a non-player move here is a teleport or a test for which the
        // snapshot is the only channel, so it still bumps.
        let non_player_changed = updates.iter().any(|update| {
            self.characters[&update.actor_id].control() != Control::Player
                && self.characters[&update.actor_id].position_m() != update.position_m
        });

        if spatial_sequence == self.spatial_sequence {
            if changed {
                return Err(SpatialUpdateError::new(
                    SpatialUpdateErrorCode::StaleSpatialSeq,
                    "spatial sequence was reused with different coordinates",
                ));
            }
            // Facings in an equal-seq repeat are deliberately NOT applied.
            return Ok(false);
        }

        for update in updates {
            let character = self
                .characters
                .get_mut(&update.actor_id)
                .expect("validated above");
            character.state.position_m = update.position_m;
            if let Some(yaw) = update.facing_yaw {
                character.state.facing_yaw = yaw;
            }
        }
        self.spatial_sequence = spatial_sequence;
        if non_player_changed {
            self.touch_public_state();
        }
        Ok(changed)
    }

    /// Advance every mover by one fixed slice and return the ids whose pose
    /// actually changed. Pure and deterministic, and — unlike [`update_positions`]
    /// — it deliberately does **not** call [`touch_public_state`]: an NPC's
    /// position rides the engine's hot `Movement` channel, never the cold public
    /// snapshot, and that split is the whole point of the fixed tick
    /// (`features/implemented/movement/06_engineering.md`). O(movers), not O(cast).
    ///
    /// `stage` is where the player stands, when the host knows: movers within
    /// [`DEFAULT_STAGE_RADIUS_M`] of it get the M7 separation steering (local
    /// avoidance is cosmetic, and nobody can tell at 200 m —
    /// `features/implemented/movement/02_navigation.md` §5). `None` (a test stepping a bare
    /// world) steers nobody.
    ///
    /// [`touch_public_state`]: World::touch_public_state
    pub fn step_movement(&mut self, dt: f64, nav: &NavData, stage: Option<Vec3>) -> Vec<ActorId> {
        // The Needle's choke, resolved from the graph (one map lookup a tick).
        let needle = nav
            .place("The Needle")
            .map(|place| (place.node, nav.node_point(place.node)));

        // Release a dead claim: the holder left the circle or stopped walking —
        // a standing body blocks nothing the separation push cannot squeeze
        // past, and holding the claim through a conversation would deadlock the
        // alley's whole neighbourhood.
        match (self.needle_claim.as_ref(), needle) {
            (Some(claim), Some((_, needle_point))) => {
                let holder_active = self.characters.get(&claim.holder).is_some_and(|holder| {
                    holder.is_walking()
                        && planar_within(holder.position_m(), needle_point, NEEDLE_CLAIM_RADIUS_M)
                });
                if !holder_active {
                    self.needle_claim = None;
                }
            }
            (Some(_), None) => self.needle_claim = None,
            (None, _) => {}
        }

        // Start-of-tick positions of everyone near the stage, for the
        // separation pass: frozen at tick start, so the push is
        // order-independent and deterministic.
        //
        // A neighbour is remembered by its *rank* in `characters` rather than by
        // its id, because an `ActorId` is a `String` and this list is rebuilt
        // from scratch on every 20 Hz slice — up to eight of them in one poll
        // after a hitch, which is exactly when nothing should be allocating.
        // The walk below is over the same unmodified `BTreeMap` in the same
        // order, so the rank identifies the same body the id did.
        let neighbours: Vec<(usize, Vec3)> = match stage {
            Some(centre) => self
                .characters
                .values()
                .enumerate()
                .filter(|(_, character)| {
                    character.state.presence == Presence::InCity
                        && planar_within(
                            character.position_m(),
                            centre,
                            DEFAULT_STAGE_RADIUS_M + AVOID_PERSONAL_RADIUS_M,
                        )
                })
                .map(|(rank, character)| (rank, character.position_m()))
                .collect(),
            None => Vec::new(),
        };

        let mut moved: Vec<ActorId> = Vec::new();
        for (rank, (id, character)) in self.characters.iter_mut().enumerate() {
            if character.state.presence != Presence::InCity || character.state.movement.is_none() {
                continue;
            }
            let start = character.state.position_m;
            let old_yaw = character.state.facing_yaw;
            let mut new_pos = start;
            let mut new_yaw = old_yaw;

            // `movement` and the position/facing fields are disjoint parts of the
            // same state; the writes below happen after this borrow ends.
            {
                let movement = character
                    .state
                    .movement
                    .as_mut()
                    .expect("checked non-None above");

                // Arrived. A patrolling mover (a scripted walk) flips its patrol and
                // routes to the far end; a mover with no patrol (the M3 water
                // round) simply stops — the behaviour ladder owns what happens on
                // arrival, not the mover (`features/implemented/movement/03_the_ladder.md` §4).
                if movement.path.is_empty()
                    && let Some(patrol) = movement.patrol.as_mut()
                {
                    patrol.heading_to_b = !patrol.heading_to_b;
                    let target = if patrol.heading_to_b {
                        &patrol.b
                    } else {
                        &patrol.a
                    };
                    match nav
                        .place(target)
                        .and_then(|place| nav.route_between(start, nav.node_point(place.node)))
                    {
                        Some(route) => {
                            // Into this walker's lane (M7), then drop the leading
                            // point when the raw route starts exactly where we
                            // stand — or the first leg is zero-length.
                            let trim = route
                                .points
                                .first()
                                .is_some_and(|point| planar_close(*point, start));
                            let mut points = nav.offset_route(&route, lane_fraction(id));
                            if trim {
                                points.remove(0);
                            }
                            movement.path = points;
                        }
                        // A missing place or an unroutable target is not a panic:
                        // the actor simply stops until the pipeline is fixed.
                        None => movement.speed = 0.0,
                    }
                }

                // Walk toward the next waypoint, in the XZ plane only — every
                // mover stays on the walk plane.
                if let Some(&waypoint) = movement.path.first() {
                    let to = Vec3::new(waypoint.x - start.x, 0.0, waypoint.z - start.z);
                    let distance = to.length();
                    let step = WALK_SPEED_MPS * dt;
                    // The tentative advance: snap onto a near waypoint (dropping
                    // it), or a full step toward it. Committed only if the
                    // Needle's claim below does not hold us at the mouth.
                    let (tentative, tentative_speed, pops_waypoint) = if distance <= step {
                        (
                            Vec3::new(waypoint.x, WALK_Y, waypoint.z),
                            if dt > 0.0 { distance / dt } else { 0.0 },
                            true,
                        )
                    } else {
                        let dir = to / distance;
                        (
                            Vec3::new(start.x + dir.x * step, WALK_Y, start.z + dir.z * step),
                            WALK_SPEED_MPS,
                            false,
                        )
                    };
                    if distance > 1e-9 {
                        let dir = to / distance;
                        // yaw 0 faces -Z, matching the rest of the codebase.
                        new_yaw = (-dir.x).atan2(-dir.z);
                    }

                    // The Needle's one-person claim (M7): stepping *into* the
                    // choke circle needs the claim — or the holder walking the
                    // same way, a follower single-file behind them. An opposing
                    // entrant waits at the mouth, facing the alley.
                    let mut blocked = false;
                    if let Some((_, needle_point)) = needle
                        && planar_within(tentative, needle_point, NEEDLE_CLAIM_RADIUS_M)
                    {
                        let inside_already =
                            planar_within(start, needle_point, NEEDLE_CLAIM_RADIUS_M);
                        let my_dir = if distance > 1e-9 {
                            to / distance
                        } else {
                            Vec3::ZERO
                        };
                        match self.needle_claim.as_mut() {
                            None => {
                                self.needle_claim = Some(NeedleClaim {
                                    holder: id.clone(),
                                    dir: my_dir,
                                });
                            }
                            Some(claim) if claim.holder == *id => claim.dir = my_dir,
                            Some(claim) => {
                                // Somebody inside without the claim (they were
                                // in the circle when it changed hands) walks on;
                                // the separation push handles the squeeze.
                                if !inside_already && claim.dir.dot(my_dir) < 0.0 {
                                    blocked = true;
                                }
                            }
                        }
                    }

                    if blocked {
                        movement.speed = 0.0;
                        movement.choke_wait += dt;
                        if movement.choke_wait >= NEEDLE_REROUTE_SECONDS {
                            // "You wait, or you take Cinder Row": the long way
                            // round, never through the needle's node. No route
                            // (the goal is *in* the choke) keeps them waiting —
                            // the claim clears the moment its holder is through.
                            movement.choke_wait = 0.0;
                            if let Some((needle_node, _)) = needle
                                && let Some(&goal) = movement.path.last()
                                && let (Some(start_node), Some(goal_node)) = (
                                    nav.nearest_node(start.x, start.z),
                                    nav.nearest_node(goal.x, goal.z),
                                )
                                && let Some(route) = nav.route_nodes_avoiding(
                                    start_node,
                                    goal_node,
                                    Some(needle_node),
                                )
                            {
                                let trim = route
                                    .points
                                    .first()
                                    .is_some_and(|point| planar_close(*point, start));
                                let mut points = nav.offset_route(&route, lane_fraction(id));
                                if trim {
                                    points.remove(0);
                                }
                                // Keep the exact original destination as the
                                // tail when it stands off the graph.
                                if !points
                                    .last()
                                    .is_some_and(|point| planar_close(*point, goal))
                                {
                                    points.push(goal);
                                }
                                movement.path = points;
                            }
                        }
                    } else {
                        movement.choke_wait = 0.0;
                        new_pos = tentative;
                        movement.speed = tentative_speed;
                        if pops_waypoint {
                            movement.path.remove(0);
                        }
                        movement.gait_phase += movement.speed * dt * GAIT_CADENCE;
                    }
                } else {
                    movement.speed = 0.0;
                }
            }

            // The M7 separation pass — on stage only, and only on a real step:
            // a bounded sideways push away from the neighbours crowding this
            // walker's personal bubble, biased to the right on a head-on meeting
            // (each walker biases to their *own* right, so the two streams break
            // apart instead of mirroring each other), clamped to walkable
            // ground so no push can put a body inside a wall.
            if new_pos != start
                && !neighbours.is_empty()
                && stage
                    .is_some_and(|centre| planar_within(new_pos, centre, DEFAULT_STAGE_RADIUS_M))
            {
                let mut push = Vec3::ZERO;
                let mut counted = 0usize;
                for (other_rank, other_pos) in &neighbours {
                    if *other_rank == rank {
                        continue;
                    }
                    let away = Vec3::new(new_pos.x - other_pos.x, 0.0, new_pos.z - other_pos.z);
                    let d = away.length();
                    if !(1e-6..AVOID_PERSONAL_RADIUS_M).contains(&d) {
                        continue;
                    }
                    push += away / d * (1.0 - d / AVOID_PERSONAL_RADIUS_M);
                    counted += 1;
                    if counted == AVOID_MAX_NEIGHBOURS {
                        break;
                    }
                }
                let push_len = push.length();
                if push_len > 1e-6 {
                    if let Some(dir) = (new_pos - start).try_normalize()
                        && push.dot(dir) < -0.7 * push_len
                    {
                        push += Vec3::new(-dir.z, 0.0, dir.x) * push_len;
                    }
                    let shove = push.clamp_length_max(AVOID_PUSH_MPS * dt);
                    let candidate = new_pos + Vec3::new(shove.x, 0.0, shove.z);
                    if nav.is_walkable(candidate.x, candidate.z) {
                        new_pos = candidate;
                    }
                }
            }

            // A step, or a pure turn — both are visible and both ride the hot
            // channel, so both count as moved.
            if new_pos != start || new_yaw != old_yaw {
                character.state.position_m = new_pos;
                character.state.facing_yaw = new_yaw;
                moved.push(id.clone());
            }
        }
        moved
    }

    pub fn public_snapshot(&self, player_id: &ActorId) -> PublicSnapshot {
        let player = self.characters.get(player_id);
        let actors = self
            .characters
            .values()
            .filter(|actor| actor.state.presence == Presence::InCity)
            .map(|actor| {
                // A missing player character shows everyone's real name.
                let known = match player {
                    None => true,
                    Some(player) => {
                        actor.id() == player.id() || player.knows().contains(actor.id())
                    }
                };
                let name_for_player = if actor.id() == player_id {
                    "You".to_string()
                } else if known {
                    actor.name().to_string()
                } else {
                    format!("a stranger (id {})", actor.id())
                };
                ActorSnapshot {
                    id: actor.id().clone(),
                    name_for_player,
                    control: actor.control(),
                    position_m: actor.position_m(),
                    facing_yaw: actor.facing_yaw(),
                    appearance: actor.appearance().clone(),
                    holds: actor.holds().to_vec(),
                    active_gesture: actor.active_gesture(),
                    statuses: actor.statuses(),
                    pockets: actor.pocket_snapshot(),
                }
            })
            .collect();
        // Who holds what, answered for the whole item table in one pass over the
        // cast. [`World::owner_of`] is a linear scan of every character, so
        // asking it per item made the snapshot O(items × cast) — with ~650
        // stacks against a 519-strong cast that is the single most expensive
        // thing a poll can do, and it lands on exactly the frames a revision
        // bumps. The map is the same shape [`World::assert_invariants`] builds,
        // and it is filled in `characters` (id) order with `or_insert`, so a
        // stack that somehow had two holders resolves to the same one
        // `owner_of` would have returned. Items are still walked in
        // `self.items` order, which is the snapshot's "sorted by id" contract.
        let mut owners: BTreeMap<&ItemId, &ActorId> = BTreeMap::new();
        for actor in self.characters.values() {
            for item_id in actor.holds() {
                owners.entry(item_id).or_insert(actor.id());
            }
        }
        let items = self
            .items
            .values()
            .filter(|item| {
                owners
                    .get(&item.id)
                    .is_some_and(|owner| self.is_present(owner))
            })
            .map(|item| ItemSnapshot {
                id: item.id.clone(),
                kind: item.kind.clone(),
                // Derived once here so the host never needs the catalog.
                display_name: self.item_catalog.display_name(item),
                display_plural: self.item_catalog.display_plural(item),
                visual_key: self.item_catalog.visual_key(item),
                quantity: item.quantity,
                metadata: item.metadata.clone(),
            })
            .collect();
        let mut offers: Vec<OfferSnapshot> = self
            .offers
            .values()
            .filter(|offer| {
                self.is_present(&offer.giver_id)
                    && offer
                        .target_id
                        .as_ref()
                        .is_none_or(|target| self.is_present(target))
            })
            .map(|offer| OfferSnapshot {
                item_id: offer.item_id.clone(),
                giver_id: offer.giver_id.clone(),
                target_id: offer.target_id.clone(),
                created_seq: offer.created_seq,
            })
            .collect();
        offers.sort_by(|left, right| {
            left.created_seq
                .cmp(&right.created_seq)
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        // The chalk. Marks whose anchor no longer resolves are simply absent —
        // the same silence the decay sweep gives them, so a home that rebinds
        // between two snapshots never publishes a mark hanging in mid-air.
        let marks = self
            .marks
            .iter()
            .filter_map(|(id, mark)| {
                let site = crate::marks::anchor_site(self, &mark.anchor)?;
                Some(crate::snapshot::PublicMark {
                    id,
                    kind: mark.kind,
                    point: [site.point.x, site.point.y, site.point.z],
                    strength_pct: crate::marks::published_strength_pct(mark.strength),
                    strokes: mark.strokes.min(u8::MAX as u32) as u8,
                })
            })
            .collect();
        PublicSnapshot {
            world_revision: self.world_revision,
            player_id: player_id.clone(),
            actors,
            items,
            offers,
            road_carts: Vec::new(),
            marks,
        }
    }

    /// The catalog display name of a stack, e.g. `broadcloth bolt of cloth`.
    pub fn item_display_name(&self, item: &Item) -> String {
        self.item_catalog.display_name(item)
    }

    /// A stack for a world dump: `spark (c0prs) ×3`, or `herring (fzbn9)` at 1.
    pub fn item_dump_label(&self, item: &Item) -> String {
        let display = self.item_catalog.display_name(item);
        if item.quantity > 1 {
            format!("{display} ({}) ×{}", item.id, item.quantity)
        } else {
            format!("{display} ({})", item.id)
        }
    }

    /// Sim-bug level checks, run after every successful offer/accept/decline/
    /// retract/eat. Panics on violation; tests call it directly.
    pub fn assert_invariants(&self) {
        let mut owners: BTreeMap<&ItemId, &ActorId> = BTreeMap::new();
        for actor in self.characters.values() {
            // The stackable same-stuff stacks this holder already carries, to
            // catch a merge that should have folded two of them into one.
            let mut seen_stuff: Vec<(&ItemId, &std::collections::BTreeMap<String, String>)> =
                Vec::new();
            for item_id in actor.holds() {
                assert!(
                    self.items.contains_key(item_id),
                    "actor {} holds missing item {item_id}",
                    actor.id()
                );
                assert!(
                    owners.insert(item_id, actor.id()).is_none(),
                    "item {item_id} has multiple owners"
                );
                let item = &self.items[item_id];
                // Quantity 0 is unrepresentable — the operation that would
                // produce it removes the stack instead.
                assert!(
                    item.quantity >= 1,
                    "item {item_id} held by {} has quantity 0",
                    actor.id()
                );
                // Metadata must be catalog-valid for a known kind (a test prop's
                // ad-hoc kind is tolerated).
                if let Err(message) = self.item_catalog.validate_item(item) {
                    panic!("{message}");
                }
                // No two same-stuff **stackable** stacks on one holder: they
                // should have merged. A non-stackable kind (a bowl of stew) may
                // legitimately appear twice.
                if self.item_catalog.stackable(item) {
                    for (other_id, other_meta) in &seen_stuff {
                        if self.items[*other_id].kind == item.kind && *other_meta == &item.metadata
                        {
                            panic!(
                                "actor {} holds two same-stuff stacks {other_id} and {item_id} \
                                 that should have merged",
                                actor.id()
                            );
                        }
                    }
                    seen_stuff.push((item_id, &item.metadata));
                }
            }
            // Body pockets (`extra_pockets.md`): every pocketed unit points at a
            // stack this same actor holds, rides a slot this body has, fits
            // (palmable), and no slot is over capacity.
            let mut slot_counts: BTreeMap<crate::character::BodySlot, usize> = BTreeMap::new();
            for unit in actor.pockets() {
                assert!(
                    actor.holds().contains(&unit.item_id),
                    "actor {} pockets {} without holding it",
                    actor.id(),
                    unit.item_id
                );
                assert!(
                    actor.has_body_slot(unit.slot),
                    "actor {} pockets {} in a {} they do not have",
                    actor.id(),
                    unit.item_id,
                    unit.slot.as_str()
                );
                let item = &self.items[&unit.item_id];
                assert!(
                    self.item_catalog.size(item) == crate::item::ItemSize::Palmable,
                    "actor {} pockets non-palmable {}",
                    actor.id(),
                    unit.item_id
                );
                *slot_counts.entry(unit.slot).or_default() += 1;
            }
            for (slot, count) in slot_counts {
                assert!(
                    count <= crate::POCKET_SLOT_CAPACITY,
                    "actor {} carries {count} units in their {} (capacity {})",
                    actor.id(),
                    slot.as_str(),
                    crate::POCKET_SLOT_CAPACITY
                );
            }
        }
        for (item_id, offer) in &self.offers {
            assert!(
                offer.item_id == *item_id,
                "offer key does not match offer item_id"
            );
            assert!(
                owners.get(item_id) == Some(&&offer.giver_id),
                "offer giver does not hold item {item_id}"
            );
            assert!(
                self.is_present(&offer.giver_id),
                "absent character {} has a live offer",
                offer.giver_id
            );
            assert!(
                offer.target_id.as_ref() != Some(&offer.giver_id),
                "offer cannot target its giver"
            );
            if let Some(target_id) = &offer.target_id {
                assert!(
                    self.is_present(target_id),
                    "offer targets a missing or absent character"
                );
            }
            let reserved = self.transform_reserved_quantity(item_id);
            let committed = reserved
                .checked_add(offer.quantity)
                .and_then(|sum| sum.checked_add(self.pocketed_quantity(item_id)))
                .expect("offer plus transform plus pocket commitment overflow");
            assert!(
                committed <= self.items[item_id].quantity,
                "item {item_id} commits {committed} units but holds only {}",
                self.items[item_id].quantity
            );
        }
        for (item_id, item) in &self.items {
            assert!(owners.contains_key(item_id), "item {item_id} has no owner");
            let reserved = self.transform_reserved_quantity(item_id);
            let offered = self.offered_quantity(item_id);
            let committed = reserved
                .checked_add(offered)
                .and_then(|sum| sum.checked_add(self.pocketed_quantity(item_id)))
                .expect("inventory commitments overflow");
            assert!(
                committed <= item.quantity,
                "item {item_id} commits {committed} units but holds only {}",
                item.quantity
            );
            let shares = self
                .legacy_restock_shares
                .get(item_id)
                .map_or(&[][..], Vec::as_slice);
            let mut previous = None;
            let share_total = shares.iter().fold(0u32, |sum, share| {
                assert!(
                    share.quantity > 0,
                    "legacy restock share on {item_id} is empty"
                );
                assert_eq!(
                    owners.get(item_id).copied(),
                    Some(&share.original_vendor),
                    "legacy restock share on {item_id} left its original vendor"
                );
                let key = (&share.source_id, &share.original_vendor);
                if let Some(old) = previous {
                    assert!(
                        old < key,
                        "legacy restock shares on {item_id} are not unique and sorted"
                    );
                }
                previous = Some(key);
                sum.checked_add(share.quantity)
                    .expect("legacy restock shares overflow")
            });
            assert!(
                share_total <= item.quantity,
                "legacy restock shares on {item_id} exceed the stack"
            );
        }
        for item_id in self.legacy_restock_shares.keys() {
            assert!(
                self.items.contains_key(item_id),
                "legacy restock provenance points at missing item {item_id}"
            );
        }
        for (producer, job) in &self.transform_jobs {
            assert_eq!(
                producer, &job.producer,
                "transform job is keyed by the wrong producer"
            );
            assert!(
                self.characters.contains_key(producer),
                "transform producer is missing"
            );
            assert!(!job.inputs.is_empty(), "transform job has no inputs");
            assert!(!job.outputs.is_empty(), "transform job has no outputs");
            assert!(
                job.progress_work_minutes.is_finite() && job.progress_work_minutes >= 0.0,
                "transform progress must be finite and non-negative"
            );
            for input in &job.inputs {
                assert!(input.quantity > 0, "transform reservation is empty");
                assert_eq!(
                    owners.get(&input.item_id).copied(),
                    Some(producer),
                    "transform input {} is not held by its producer",
                    input.item_id
                );
            }
            let mut output_keys = BTreeMap::<crate::inventory::ItemMatcher, u32>::new();
            for output in &job.outputs {
                assert!(output.quantity > 0, "transform output is empty");
                let total = output_keys.entry(output.matcher()).or_default();
                *total = total
                    .checked_add(output.quantity)
                    .expect("transform output overflow");
            }
            for (matcher, future) in output_keys {
                self.held_quantity(producer, &matcher)
                    .checked_add(future)
                    .expect("held stock exceeds reserved transform output capacity");
            }
        }
    }
}

/// Whether two points coincide on the walk plane (XZ), within a hair. Used to
/// drop a route's leading point when it is exactly where the mover stands.
pub(crate) fn planar_close(a: Vec3, b: Vec3) -> bool {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz < 1e-9
}

/// Whether `a` lies within `radius` of `b` on the walk plane (XZ).
pub(crate) fn planar_within(a: Vec3, b: Vec3, radius: f64) -> bool {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz <= radius * radius
}

/// A fresh, deterministic 5-char item id for a partial-split residue, hashed
/// from `(parent_id, salt)` where `salt` is the unique event sequence of the
/// accept that created it — so headless runs stay reproducible and never
/// collide (`01_items_and_stacks.md` §6). Rendered in the base-32 style the cast
/// already uses.
pub fn mint_item_id(parent: &ItemId, salt: i64) -> ItemId {
    const ALPHABET: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";
    let mut hasher = DefaultHasher::new();
    "item_split".hash(&mut hasher);
    parent.as_str().hash(&mut hasher);
    salt.hash(&mut hasher);
    let mut value = hasher.finish();
    let mut chars = [0u8; 5];
    for slot in chars.iter_mut() {
        *slot = ALPHABET[(value & 31) as usize];
        value >>= 5;
    }
    ItemId::from_raw(String::from_utf8(chars.to_vec()).expect("ASCII alphabet"))
}

/// A pure `[0, 1)` roll from `(salt, actor_id, epoch)` — the sim's deterministic
/// stand-in for an RNG (`attention.rs` curiosity idiom). Shared by the round's
/// ladder and the mover's lane assignment.
pub(crate) fn hash01(salt: &str, id: &ActorId, epoch: u64) -> f64 {
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    id.as_str().hash(&mut hasher);
    epoch.hash(&mut hasher);
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

/// A walker's stable lane (M7): keep right of the crown by
/// [`LANE_KEEP_RIGHT_FRACTION`] of the usable corridor, jittered per actor by
/// ±[`LANE_JITTER_FRACTION`] — always positive, so nobody walks against the
/// stream, and stable for life, so the same man holds the same line every day.
pub(crate) fn lane_fraction(id: &ActorId) -> f64 {
    LANE_KEEP_RIGHT_FRACTION + (hash01("lane", id, 0) - 0.5) * 2.0 * LANE_JITTER_FRACTION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HEARING_RADIUS_M, WALK_SPEED_MPS,
        character::{CharacterSheet, Control, Movement, Patrol},
    };
    use std::collections::BTreeSet;

    fn character(id: &str, x: f64) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        })
    }

    #[test]
    fn hearing_boundary_is_inclusive_and_ordered_by_distance() {
        let mut world = World::new();
        world.add_character(character("origin", 0.0));
        world.add_character(character("inside", HEARING_RADIUS_M - 1e-6));
        world.add_character(character("exact", HEARING_RADIUS_M));
        world.add_character(character("outside", HEARING_RADIUS_M + 1e-6));

        let origin = ActorId::from_raw("origin");
        let heard = world.characters_within(Vec3::ZERO, HEARING_RADIUS_M, Some(&origin));

        assert_eq!(
            heard,
            vec![ActorId::from_raw("inside"), ActorId::from_raw("exact")]
        );
    }

    #[test]
    fn equal_distance_ties_break_by_id() {
        let mut world = World::new();
        world.add_character(character("origin", 0.0));
        world.add_character(character("z", 2.0));
        world.add_character(character("b", 1.0));
        world.add_character(character("a", -1.0));

        let origin = ActorId::from_raw("origin");
        let heard = world.characters_within(Vec3::ZERO, 20.0, Some(&origin));

        assert_eq!(
            heard,
            vec![
                ActorId::from_raw("a"),
                ActorId::from_raw("b"),
                ActorId::from_raw("z")
            ]
        );
    }

    /// The borrowing sibling is the *same* answer, not a similar one.
    ///
    /// `characters_within`'s distance-then-id order fixes `recipient_ids` in
    /// every event and the nearest witness the scheduler nudges, and callers
    /// now pick whichever form allocates less — so the two must never be able
    /// to drift apart, including on the tie, the exclusion and the boundary.
    #[test]
    fn the_borrowed_neighbourhood_is_the_owned_one() {
        let mut world = World::new();
        world.add_character(character("origin", 0.0));
        world.add_character(character("z", 2.0));
        world.add_character(character("b", 1.0));
        world.add_character(character("a", -1.0));
        world.add_character(character("far", HEARING_RADIUS_M + 1e-6));
        let mut absent = character("gone", 0.5);
        absent.state.presence = Presence::BeyondTheWalls;
        world.add_character(absent);

        let origin = ActorId::from_raw("origin");
        for exclude in [None, Some(&origin)] {
            for radius in [0.0, 1.0, HEARING_RADIUS_M, 1_000.0] {
                let owned = world.characters_within(Vec3::ZERO, radius, exclude);
                let borrowed: Vec<ActorId> = world
                    .characters_within_refs(Vec3::ZERO, radius, exclude)
                    .into_iter()
                    .cloned()
                    .collect();
                assert_eq!(owned, borrowed, "radius {radius}, exclude {exclude:?}");
            }
        }
    }

    #[test]
    fn roster_keeps_insertion_order_while_the_map_sorts() {
        let mut world = World::new();
        for id in ["sv3n1", "cb947", "k0fb1"] {
            world.add_character(character(id, 0.0));
        }
        assert_eq!(
            world.roster,
            vec![
                ActorId::from_raw("sv3n1"),
                ActorId::from_raw("cb947"),
                ActorId::from_raw("k0fb1")
            ]
        );
        let sorted: Vec<&str> = world.characters.keys().map(ActorId::as_str).collect();
        assert_eq!(sorted, ["cb947", "k0fb1", "sv3n1"]);
    }

    #[test]
    fn facing_only_updates_never_bump_the_revision() {
        let mut world = World::new();
        world.add_character(character("actor", 0.0));
        let revision = world.world_revision;
        let actor = ActorId::from_raw("actor");

        let changed = world
            .update_positions(
                1,
                &[SpatialActorUpdate::new(
                    actor.clone(),
                    Vec3::new(0.0, 0.0, 0.0),
                    Some(1.0),
                )],
            )
            .unwrap();

        assert!(!changed);
        assert_eq!(world.world_revision, revision);
        assert_eq!(world.characters[&actor].facing_yaw(), 1.0);
    }

    #[test]
    fn a_player_move_stays_off_the_cold_channel_but_an_npc_move_bumps_it() {
        let mut world = World::new();
        let mut player = character("plyr1", 0.0);
        player.sheet.control = Control::Player;
        world.add_character(player);
        world.add_character(character("np001", 5.0));
        let player_id = ActorId::from_raw("plyr1");
        let npc_id = ActorId::from_raw("np001");

        // The player walking is applied to live state but never republishes the
        // cold snapshot: the host owns the player's transform.
        let revision = world.world_revision;
        let changed = world
            .update_positions(
                1,
                &[SpatialActorUpdate::new(
                    player_id.clone(),
                    Vec3::new(1.0, 0.0, 0.0),
                    None,
                )],
            )
            .unwrap();
        assert!(changed, "the position was applied");
        assert_eq!(
            world.characters[&player_id].position_m(),
            Vec3::new(1.0, 0.0, 0.0)
        );
        assert_eq!(
            world.world_revision, revision,
            "a player-only move must not bump the public revision"
        );

        // Moving a non-player actor is a teleport the snapshot is the only
        // channel for, so it still bumps.
        let changed = world
            .update_positions(
                2,
                &[SpatialActorUpdate::new(
                    npc_id.clone(),
                    Vec3::new(7.0, 0.0, 0.0),
                    None,
                )],
            )
            .unwrap();
        assert!(changed);
        assert_eq!(
            world.world_revision,
            revision + 1,
            "a non-player move still republishes the cold snapshot"
        );
    }

    #[test]
    fn equal_sequence_repeats_must_match_and_reject_non_finite_yaw() {
        let mut world = World::new();
        world.add_character(character("actor", 0.0));
        let actor = ActorId::from_raw("actor");

        world
            .update_positions(
                1,
                &[SpatialActorUpdate::new(
                    actor.clone(),
                    Vec3::new(1.0, 0.0, 0.0),
                    Some(2.5),
                )],
            )
            .unwrap();
        assert_eq!(world.characters[&actor].facing_yaw(), 2.5);

        // Same seq, same coordinates: idempotent no-op.
        assert!(
            !world
                .update_positions(
                    1,
                    &[SpatialActorUpdate::new(
                        actor.clone(),
                        Vec3::new(1.0, 0.0, 0.0),
                        None
                    )]
                )
                .unwrap()
        );
        // Same seq, different coordinates: stale.
        let error = world
            .update_positions(
                1,
                &[SpatialActorUpdate::new(
                    actor.clone(),
                    Vec3::new(2.0, 0.0, 0.0),
                    None,
                )],
            )
            .unwrap_err();
        assert_eq!(error.code, SpatialUpdateErrorCode::StaleSpatialSeq);

        let error = world
            .update_positions(
                2,
                &[SpatialActorUpdate::new(
                    actor,
                    Vec3::new(1.0, 0.0, 0.0),
                    Some(f64::NAN),
                )],
            )
            .unwrap_err();
        assert_eq!(error.code, SpatialUpdateErrorCode::InvalidPosition);
    }

    // ------------------------------------------------------------- movement

    /// A tiny hand-built graph: four nodes 10 m apart along +x, welded into one
    /// straight corridor, with a named place at each end. All-walkable bitset —
    /// `step_movement` never consults it, only [`NavData::from_parts`] validates
    /// its length.
    fn line_nav() -> NavData {
        let w = 60usize;
        let h = 10usize;
        let bytes = (w * h).div_ceil(8);
        let bitset = vec![0xFF_u8; bytes];
        let json = format!(
            r#"{{
              "schema_version": 1,
              "grid": {{"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": {w}, "h": {h},
                        "agent_radius_m": 0.35, "bitset_file": "x.bin",
                        "bitset_bits": {bits}, "bitset_sha256": ""}},
              "nodes": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]],
              "edges": [[0, 1, 2.0], [1, 2, 2.0], [2, 3, 2.0]],
              "places": [{{"name": "a", "node": 0, "kind": "place"}},
                         {{"name": "b", "node": 3, "kind": "place"}}],
              "sites": [],
              "doors": [],
              "reference": {{"forecourt": 0}}
            }}"#,
            bits = w * h
        );
        NavData::from_parts(&json, &bitset).expect("the hand-built nav validates")
    }

    /// A mover parked at place `a`, already routed toward place `b`.
    fn walker(nav: &NavData) -> Character {
        let start = nav.node_point(nav.place("a").unwrap().node);
        let mut character = Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw("walker"),
            name: "WALKER".into(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "the corridor".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: start,
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        });
        let target = nav.node_point(nav.place("b").unwrap().node);
        let mut path = nav.route_between(start, target).unwrap().points;
        if path.first().is_some_and(|p| planar_close(*p, start)) {
            path.remove(0);
        }
        character.state.movement = Some(Movement {
            path,
            speed: WALK_SPEED_MPS,
            gait_phase: 0.0,
            patrol: Some(Patrol {
                a: "a".into(),
                b: "b".into(),
                heading_to_b: true,
            }),
            choke_wait: 0.0,
        });
        character
    }

    fn walker_world() -> (World, NavData, ActorId) {
        let nav = line_nav();
        let mut world = World::new();
        world.add_character(walker(&nav));
        let id = ActorId::from_raw("walker");
        (world, nav, id)
    }

    const TICK: f64 = 0.05;

    #[test]
    fn a_mover_advances_one_walk_step_per_slice() {
        let (mut world, nav, id) = walker_world();

        let revision = world.world_revision;
        let moved = world.step_movement(TICK, &nav, None);
        assert_eq!(moved, vec![id.clone()]);
        // A movement slice never touches the public revision (the hot/cold split).
        assert_eq!(world.world_revision, revision);

        let character = &world.characters[&id];
        // Exactly WALK_SPEED_MPS * dt along +x, still on the walk plane.
        assert!((character.position_m().x - WALK_SPEED_MPS * TICK).abs() < 1e-12);
        assert!(character.position_m().z.abs() < 1e-12);
        assert!((character.position_m().y - WALK_Y).abs() < 1e-12);
        assert_eq!(character.speed(), WALK_SPEED_MPS);
        // yaw 0 faces -Z; heading +x is a quarter turn to -π/2.
        assert!((character.facing_yaw() - (-std::f64::consts::FRAC_PI_2)).abs() < 1e-12);
        assert!(character.state.movement.as_ref().unwrap().gait_phase > 0.0);
    }

    #[test]
    fn a_mover_arrives_within_epsilon_then_reverses_and_reroutes() {
        let (mut world, nav, id) = walker_world();
        let goal = nav.node_point(nav.place("b").unwrap().node);

        // Walk until the path empties — arrival. 30 m at 0.09 m/slice is ~340.
        let mut slices = 0;
        while !world.characters[&id]
            .state
            .movement
            .as_ref()
            .unwrap()
            .path
            .is_empty()
        {
            world.step_movement(TICK, &nav, None);
            slices += 1;
            assert!(slices < 500, "the mover never arrived");
        }
        let arrival = world.characters[&id].position_m();
        assert!(
            arrival.distance(goal) < 1e-9,
            "arrived at {arrival:?}, wanted {goal:?}"
        );

        // One more slice: the patrol flips and routes back toward `a`.
        world.step_movement(TICK, &nav, None);
        let movement = world.characters[&id].state.movement.as_ref().unwrap();
        assert!(
            !movement.patrol.as_ref().unwrap().heading_to_b,
            "the patrol reversed on arrival"
        );
        assert!(
            !movement.path.is_empty(),
            "a fresh route back to `a` was laid"
        );
        assert!(
            world.characters[&id].position_m().x < goal.x,
            "the mover is now walking back the way it came"
        );
    }

    /// The whole route's length falls out of the per-slice sum, so the mover walks
    /// the graph distance and no more — the acceptance that positions are metres.
    #[test]
    fn the_total_walk_matches_the_route_length() {
        let (mut world, nav, id) = walker_world();
        let start = world.characters[&id].position_m();
        let route_length = nav
            .route_between(start, nav.node_point(nav.place("b").unwrap().node))
            .unwrap()
            .length_m;

        let mut travelled = 0.0;
        let mut previous = start;
        for _ in 0..500 {
            world.step_movement(TICK, &nav, None);
            let now = world.characters[&id].position_m();
            travelled += previous.distance(now);
            previous = now;
            if world.characters[&id]
                .state
                .movement
                .as_ref()
                .unwrap()
                .path
                .is_empty()
            {
                break;
            }
        }
        assert!(
            (travelled - route_length).abs() < 1e-6,
            "walked {travelled} m, route is {route_length} m"
        );
    }

    #[test]
    fn a_character_without_movement_never_moves() {
        let nav = line_nav();
        let mut world = World::new();
        world.add_character(character("still", 7.0));
        let before = world.characters[&ActorId::from_raw("still")].position_m();
        assert!(world.step_movement(TICK, &nav, None).is_empty());
        assert_eq!(
            world.characters[&ActorId::from_raw("still")].position_m(),
            before
        );
    }

    // ------------------------------------------------------- M7: the crowd

    /// A mover with a hand-laid path (no patrol, no lane shift), for the
    /// avoidance and Needle tests where the geometry must be exact.
    fn mover(id: &str, from: Vec3, path: Vec<Vec3>) -> Character {
        let mut character = character(id, 0.0);
        character.state.position_m = from;
        character.state.movement = Some(Movement {
            path,
            speed: WALK_SPEED_MPS,
            gait_phase: 0.0,
            patrol: None,
            choke_wait: 0.0,
        });
        character
    }

    #[test]
    fn movers_hold_distinct_right_hand_lanes() {
        let nav = line_nav();
        let start = nav.node_point(0);
        let goal = nav.node_point(3);
        let route = nav.route_between(start, goal).expect("the line routes");

        let lane_a = lane_fraction(&ActorId::from_raw("walker"));
        let lane_b = lane_fraction(&ActorId::from_raw("other"));
        let path_a = nav.offset_route(&route, lane_a);
        let path_b = nav.offset_route(&route, lane_b);

        // Heading +x, the walker's right is +z: every shifted vertex sits in
        // the right half of the corridor, within the usable half-width
        // (2.0 m edge half-width minus the 0.35 m agent radius).
        for point in &path_a[..path_a.len() - 1] {
            assert!(point.z > 0.0, "the lane keeps right, got z={}", point.z);
            assert!(
                point.z <= 2.0 - 0.35 + 1e-9,
                "the lane stays in the corridor"
            );
        }
        // Two walkers hold different, stable lines — no conga line.
        assert!(
            (path_a[1].z - path_b[1].z).abs() > 1e-3,
            "distinct actors should hold distinct lanes"
        );
        assert_eq!(lane_a, lane_fraction(&ActorId::from_raw("walker")));
        // The final vertex is exact: arrival semantics stay point-precise.
        assert_eq!(path_a.last(), route.points.last());
    }

    /// Two movers crossing on the centreline: off stage they pass through each
    /// other (avoidance is cosmetic and costs nothing at distance); on stage
    /// the separation push opens a gap — and both still arrive.
    #[test]
    fn on_stage_movers_separate_where_off_stage_they_pass_through() {
        let crossing = || {
            let nav = line_nav();
            let mut world = World::new();
            world.add_character(mover(
                "east",
                Vec3::new(0.0, WALK_Y, 0.0),
                vec![Vec3::new(30.0, WALK_Y, 0.0)],
            ));
            world.add_character(mover(
                "west",
                Vec3::new(30.0, WALK_Y, 0.0),
                vec![Vec3::new(0.0, WALK_Y, 0.0)],
            ));
            (
                world,
                nav,
                ActorId::from_raw("east"),
                ActorId::from_raw("west"),
            )
        };
        let closest = |world: &World, a: &ActorId, b: &ActorId| {
            world.characters[a]
                .position_m()
                .distance(world.characters[b].position_m())
        };

        let (mut world, nav, east, west) = crossing();
        let mut min_off_stage = f64::INFINITY;
        for _ in 0..600 {
            world.step_movement(TICK, &nav, None);
            min_off_stage = min_off_stage.min(closest(&world, &east, &west));
        }
        assert!(
            min_off_stage < 0.2,
            "unsteered movers cross on the centreline (min {min_off_stage:.2} m)"
        );

        let (mut world, nav, east, west) = crossing();
        let stage = Some(Vec3::new(15.0, WALK_Y, 0.0));
        let mut min_on_stage = f64::INFINITY;
        for _ in 0..600 {
            world.step_movement(TICK, &nav, stage);
            min_on_stage = min_on_stage.min(closest(&world, &east, &west));
        }
        assert!(
            min_on_stage > 0.45,
            "on stage the separation push opens a gap (min {min_on_stage:.2} m)"
        );
        for id in [&east, &west] {
            assert!(
                !world.characters[id].is_walking(),
                "steered movers still arrive"
            );
        }
    }

    /// A diamond around a one-person choke: `west (0,0) — needle (30,0) —
    /// east (50,0)` on 0.6 m alley edges, with a wide detour over `(30,30)`.
    fn needle_nav() -> NavData {
        let w = 70usize;
        let h = 45usize;
        let bytes = (w * h).div_ceil(8);
        let bitset = vec![0xFF_u8; bytes];
        let json = format!(
            r#"{{
              "schema_version": 1,
              "grid": {{"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": {w}, "h": {h},
                        "agent_radius_m": 0.35, "bitset_file": "x.bin",
                        "bitset_bits": {bits}, "bitset_sha256": ""}},
              "nodes": [[0.0, 0.0], [30.0, 0.0], [50.0, 0.0], [30.0, 30.0]],
              "edges": [[0, 1, 0.6], [1, 2, 0.6], [0, 3, 2.0], [3, 2, 2.0]],
              "places": [{{"name": "west", "node": 0, "kind": "place"}},
                         {{"name": "east", "node": 2, "kind": "place"}},
                         {{"name": "The Needle", "node": 1, "kind": "route"}}],
              "sites": [],
              "doors": [],
              "reference": {{"forecourt": 0}}
            }}"#,
            bits = w * h
        );
        NavData::from_parts(&json, &bitset).expect("the diamond nav validates")
    }

    #[test]
    fn route_nodes_avoiding_takes_the_detour() {
        let nav = needle_nav();
        let direct = nav.route_nodes(2, 0).expect("the alley routes");
        assert!(
            direct.nodes.contains(&1),
            "the short way runs through the Needle"
        );
        let detour = nav
            .route_nodes_avoiding(2, 0, Some(1))
            .expect("the long way round exists");
        assert_eq!(detour.nodes, vec![2, 3, 0]);
        // A goal *in* the choke has no route around it.
        assert!(nav.route_nodes_avoiding(2, 1, Some(1)).is_none());
    }

    #[test]
    fn an_opposing_walker_waits_at_the_needles_mouth() {
        let nav = needle_nav();
        let mut world = World::new();
        // Eastbound, already inside the choke circle: takes the claim.
        world.add_character(mover(
            "hold1",
            Vec3::new(20.0, WALK_Y, 0.0),
            vec![Vec3::new(30.0, WALK_Y, 0.0), Vec3::new(50.0, WALK_Y, 0.0)],
        ));
        // Westbound: must stand at the mouth until the holder is through.
        world.add_character(mover(
            "waiter",
            Vec3::new(50.0, WALK_Y, 0.0),
            vec![Vec3::new(30.0, WALK_Y, 0.0), Vec3::new(0.0, WALK_Y, 0.0)],
        ));
        let hold1 = ActorId::from_raw("hold1");
        let waiter = ActorId::from_raw("waiter");

        let mut waiter_min_x_while_held = f64::INFINITY;
        let mut held_at_all = false;
        for _ in 0..900 {
            world.step_movement(TICK, &nav, None);
            if world
                .needle_claim
                .as_ref()
                .is_some_and(|claim| claim.holder == hold1)
            {
                held_at_all = true;
                waiter_min_x_while_held =
                    waiter_min_x_while_held.min(world.characters[&waiter].position_m().x);
            }
        }
        assert!(held_at_all, "the eastbound walker claims the choke");
        // The choke circle is 14 m around (30, 0): the mouth is x = 44.
        assert!(
            waiter_min_x_while_held > 43.5,
            "the opposing walker stood at the mouth (min x {waiter_min_x_while_held:.2})"
        );
        // Both got where they were going once the alley cleared.
        assert!(planar_close(
            world.characters[&hold1].position_m(),
            Vec3::new(50.0, WALK_Y, 0.0)
        ));
        assert!(planar_close(
            world.characters[&waiter].position_m(),
            Vec3::new(0.0, WALK_Y, 0.0)
        ));
    }

    #[test]
    fn a_long_wait_takes_the_long_way_round() {
        let nav = needle_nav();
        let mut world = World::new();
        // Two eastbound walkers in file hold the choke long enough that the
        // westbound walker's patience (NEEDLE_REROUTE_SECONDS) runs out.
        world.add_character(mover(
            "hold1",
            Vec3::new(20.0, WALK_Y, 0.0),
            vec![Vec3::new(30.0, WALK_Y, 0.0), Vec3::new(50.0, WALK_Y, 0.0)],
        ));
        world.add_character(mover(
            "hold2",
            Vec3::new(-6.0, WALK_Y, 0.0),
            vec![Vec3::new(30.0, WALK_Y, 0.0), Vec3::new(50.0, WALK_Y, 0.0)],
        ));
        world.add_character(mover(
            "waiter",
            Vec3::new(50.0, WALK_Y, 0.0),
            vec![Vec3::new(30.0, WALK_Y, 0.0), Vec3::new(0.0, WALK_Y, 0.0)],
        ));
        let waiter = ActorId::from_raw("waiter");
        let needle = Vec3::new(30.0, WALK_Y, 0.0);

        // Whole-second slices, so the wait budget runs out while the second
        // holder is still inside.
        let mut rerouted = false;
        for _ in 0..25 {
            world.step_movement(1.0, &nav, None);
            let path = &world.characters[&waiter]
                .state
                .movement
                .as_ref()
                .unwrap()
                .path;
            if path.iter().any(|point| point.z > 20.0) {
                rerouted = true;
                break;
            }
        }
        assert!(
            rerouted,
            "the waiter gave up on the mouth and took the detour"
        );

        // The detour never re-enters the choke, and it still gets home.
        let mut min_needle_distance = f64::INFINITY;
        for _ in 0..80 {
            world.step_movement(1.0, &nav, None);
            let position = world.characters[&waiter].position_m();
            min_needle_distance = min_needle_distance.min(
                Vec3::new(position.x, 0.0, position.z).distance(Vec3::new(needle.x, 0.0, needle.z)),
            );
            if !world.characters[&waiter].is_walking() {
                break;
            }
        }
        assert!(
            min_needle_distance > NEEDLE_CLAIM_RADIUS_M,
            "the long way round stays out of the choke (min {min_needle_distance:.1} m)"
        );
        assert!(
            planar_close(
                world.characters[&waiter].position_m(),
                Vec3::new(0.0, WALK_Y, 0.0)
            ),
            "the rerouted walker still arrives"
        );
    }

    // ------------------------------------------------- §8 carriage statuses

    /// The debug carriage write finds a character by display name
    /// (case-insensitive), clamps the value, bumps the revision, and refuses a
    /// name that matches nobody — the sole non-test writer of `statuses`.
    #[test]
    fn debug_set_status_writes_by_name_clamps_and_bumps_revision() {
        let mut world = World::new();
        world.add_character(character("ilse", 0.0)); // display name "ILSE"
        let revision = world.world_revision;

        assert!(world.debug_set_status("Ilse", StatusKind::Drunkenness, 1.4));
        assert!(
            world.world_revision > revision,
            "a status write must republish the snapshot"
        );
        let ilse = &world.characters[&ActorId::from_raw("ilse")];
        assert_eq!(
            ilse.state.statuses[&StatusKind::Drunkenness],
            1.0,
            "clamped"
        );

        // A non-finite value reads as no carriage.
        assert!(world.debug_set_status("ILSE", StatusKind::Weariness, f64::INFINITY));
        assert_eq!(
            world.characters[&ActorId::from_raw("ilse")].state.statuses[&StatusKind::Weariness],
            0.0
        );

        // Nobody by that name: no write, no revision bump.
        let revision = world.world_revision;
        assert!(!world.debug_set_status("Nobody", StatusKind::Drunkenness, 0.5));
        assert_eq!(world.world_revision, revision);
    }

    /// The debug status poke also resolves a target by actor id (what the HUD
    /// shows for a stranger) and by an `<id>_<name>` lore stem, not just by name.
    #[test]
    fn debug_set_status_resolves_by_id_and_stem_too() {
        let mut world = World::new();
        // A display name deliberately unlike the id, so a name hit and an id hit
        // are distinguishable.
        world.add_character(Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw("p006v"),
            name: "Wend Carrow".into(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: Vec3::new(0.0, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        }));
        let id = ActorId::from_raw("p006v");

        // By display name (case-insensitive, may contain spaces).
        assert!(world.debug_set_status("wend carrow", StatusKind::Drunkenness, 0.4));
        assert_eq!(
            world.characters[&id].state.statuses[&StatusKind::Drunkenness],
            0.4
        );

        // By the raw actor id the HUD shows for strangers.
        assert!(world.debug_set_status("P006V", StatusKind::Weariness, 0.6));
        assert_eq!(
            world.characters[&id].state.statuses[&StatusKind::Weariness],
            0.6
        );

        // By an `<id>_<name>` stem someone might paste from a lore path.
        assert!(world.debug_set_status("p006v_wend", StatusKind::Drunkenness, 0.9));
        assert_eq!(
            world.characters[&id].state.statuses[&StatusKind::Drunkenness],
            0.9
        );

        // Neither a name nor an id: no write.
        assert!(!world.debug_set_status("q9zzz", StatusKind::Drunkenness, 0.5));
    }

    /// Statuses surface on the public snapshot, clamped and ordered by kind, and
    /// the snapshot round-trips through serde (empty stays empty and absent).
    #[test]
    fn public_snapshot_exposes_statuses_and_round_trips() {
        let mut world = World::new();
        world.add_character(character("player", 0.0));
        world.add_character(character("sv3n1", 1.0));
        world.debug_set_status("SV3N1", StatusKind::Weariness, 0.5);
        world.debug_set_status("SV3N1", StatusKind::Drunkenness, 0.75);

        let snapshot = world.public_snapshot(&ActorId::from_raw("player"));
        let sven = snapshot
            .actors
            .iter()
            .find(|actor| actor.id == ActorId::from_raw("sv3n1"))
            .expect("sven is in the snapshot");
        // BTreeMap order: Drunkenness before Weariness.
        assert_eq!(
            sven.statuses,
            vec![
                (StatusKind::Drunkenness, 0.75),
                (StatusKind::Weariness, 0.5)
            ]
        );
        let player = snapshot
            .actors
            .iter()
            .find(|actor| actor.id == ActorId::from_raw("player"))
            .expect("the player is in the snapshot");
        assert!(
            player.statuses.is_empty(),
            "an untouched actor carries none"
        );

        // Round-trips, and the empty case is skipped (never `"statuses":[]`).
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(
            !encoded.contains("\"statuses\":[]"),
            "empty statuses must be skipped: {encoded}"
        );
        let decoded: PublicSnapshot = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, snapshot);
    }
}
