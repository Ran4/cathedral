//! The world (`sim.py:159-389`): entities, the event buffer, spatial updates,
//! the public snapshot, and the invariants a sim bug would break.
//!
//! Maps are `BTreeMap` (deterministic iteration; Rust `String` Ord is UTF-8
//! byte order, which equals Python's code-point order). Round-robin scheduling
//! needs *insertion* order instead, so [`World::roster`] records it (D12).

use std::collections::BTreeMap;

use crate::{
    DEFAULT_VIEW_CONE_DEGREES,
    areas::AreaMap,
    character::Character,
    error::{SpatialUpdateError, SpatialUpdateErrorCode},
    event::DomainEvent,
    ids::{ActorId, ItemId},
    item::Item,
    math::Vec3,
    offer::Offer,
    snapshot::{ActorSnapshot, ItemSnapshot, OfferSnapshot, PublicSnapshot},
    sounds::SoundCatalog,
};

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

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    /// Authoritative named geography used whenever a prompt is rendered.
    pub area_map: AreaMap,
    pub characters: BTreeMap<ActorId, Character>,
    /// Character insertion order — the round-robin turn order (D12).
    pub roster: Vec<ActorId>,
    pub items: BTreeMap<ItemId, Item>,
    /// Keyed by item id: at most one live offer per item.
    pub offers: BTreeMap<ItemId, Offer>,
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
    events: Vec<DomainEvent>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            area_map: AreaMap::default(),
            characters: BTreeMap::new(),
            roster: Vec::new(),
            items: BTreeMap::new(),
            offers: BTreeMap::new(),
            world_revision: 0,
            event_sequence: 0,
            spatial_sequence: -1,
            sounds_enabled: true,
            view_cone_degrees: DEFAULT_VIEW_CONE_DEGREES,
            sound_catalog: SoundCatalog::empty(),
            events: Vec::new(),
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
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

    pub fn touch_public_state(&mut self) -> i64 {
        self.world_revision += 1;
        self.world_revision
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
        assert!(
            radius.is_finite() && radius >= 0.0,
            "radius must be a finite non-negative number"
        );
        let radius_squared = radius * radius;
        let mut matches: Vec<(f64, &ActorId)> = self
            .characters
            .values()
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
        matches.into_iter().map(|(_, id)| id.clone()).collect()
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
    /// next snapshot always reads current facing). Returns whether any position
    /// changed.
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
        if changed {
            self.touch_public_state();
        }
        Ok(changed)
    }

    pub fn public_snapshot(&self, player_id: &ActorId) -> PublicSnapshot {
        let player = self.characters.get(player_id);
        let actors = self
            .characters
            .values()
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
                    appearance_key: actor.appearance_key().to_string(),
                    holds: actor.holds().to_vec(),
                }
            })
            .collect();
        let items = self
            .items
            .values()
            .map(|item| ItemSnapshot {
                id: item.id.clone(),
                name: item.name.clone(),
                visual_key: item.visual_key.clone(),
            })
            .collect();
        let mut offers: Vec<OfferSnapshot> = self
            .offers
            .values()
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
        PublicSnapshot {
            world_revision: self.world_revision,
            player_id: player_id.clone(),
            actors,
            items,
            offers,
        }
    }

    /// Sim-bug level checks, run after every successful offer/accept/decline/
    /// retract/eat. Panics on violation; tests call it directly.
    pub fn assert_invariants(&self) {
        let mut owners: BTreeMap<&ItemId, &ActorId> = BTreeMap::new();
        for actor in self.characters.values() {
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
                offer.target_id.as_ref() != Some(&offer.giver_id),
                "offer cannot target its giver"
            );
            if let Some(target_id) = &offer.target_id {
                assert!(
                    self.characters.contains_key(target_id),
                    "offer targets a missing character"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HEARING_RADIUS_M,
        character::{CharacterSheet, Control},
    };
    use std::collections::BTreeSet;

    fn character(id: &str, x: f64) -> Character {
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance_key: id.into(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: "None".into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
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
}
