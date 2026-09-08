//! M2a2 component composition, deliberately not a World/Engine save. All fields
//! here are mandatory. Missing Round, law, knowledge, scheduler and other city
//! owners must be composed and cross-validated before any complete hydration.
use super::*;
use crate::{
    checkpoint::{
        Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate, records,
    },
    inventory::checkpoint::{InventoryDtoV1, View as InventoryView},
    places::checkpoint::PlaceRegistryV1,
    receipts::{CommandId, CommandLedger, ReceiptState},
};
use serde::{Deserialize, Serialize};
const OWNER: &str = "world_backbone";
pub const MAX_CHARACTERS: usize = 25_000;
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorldBackboneDtoV1 {
    version: u16,
    #[serde(with = "records::character::map")]
    characters: BTreeMap<ActorId, Character>,
    roster: Vec<ActorId>,
    inventory: InventoryDtoV1,
    #[serde(with = "PlaceRegistryV1")]
    places: PlaceRegistry,
    #[serde(with = "records::point::map")]
    household_doors: BTreeMap<ActorId, Vec3>,
    #[serde(with = "records::command::map")]
    round_actions: BTreeMap<ActorId, CommandId>,
    #[serde(with = "records::command::map")]
    travel_actions: BTreeMap<ActorId, CommandId>,
    world_revision: i64,
    event_sequence: i64,
    spatial_sequence: i64,
    sounds_enabled: bool,
    view_cone_degrees: f64,
    #[serde(with = "records::world_time::option")]
    current_time: Option<WorldTime>,
    #[serde(with = "records::needle::option")]
    needle_claim: Option<NeedleClaim>,
    #[serde(deserialize_with = "crate::checkpoint::serde_support::required_option")]
    spoke_this_turn: Option<ActorId>,
}
// Deserialization is private so external callers cannot bypass aggregate
// admission with serde_json::from_slice::<PublicDto>().
#[derive(Serialize, Deserialize)]
#[serde(remote = "WorldBackboneDtoV1", deny_unknown_fields)]
pub(crate) struct WorldBackboneWireV1 {
    version: u16,
    #[serde(with = "records::character::map")]
    characters: BTreeMap<ActorId, Character>,
    roster: Vec<ActorId>,
    #[serde(with = "crate::inventory::checkpoint::InventoryWireV1")]
    inventory: InventoryDtoV1,
    #[serde(with = "PlaceRegistryV1")]
    places: PlaceRegistry,
    #[serde(with = "records::point::map")]
    household_doors: BTreeMap<ActorId, Vec3>,
    #[serde(with = "records::command::map")]
    round_actions: BTreeMap<ActorId, CommandId>,
    #[serde(with = "records::command::map")]
    travel_actions: BTreeMap<ActorId, CommandId>,
    world_revision: i64,
    event_sequence: i64,
    spatial_sequence: i64,
    sounds_enabled: bool,
    view_cone_degrees: f64,
    #[serde(with = "records::world_time::option")]
    current_time: Option<WorldTime>,
    #[serde(with = "records::needle::option")]
    needle_claim: Option<NeedleClaim>,
    #[serde(deserialize_with = "crate::checkpoint::serde_support::required_option")]
    spoke_this_turn: Option<ActorId>,
}
#[derive(Deserialize)]
struct Decoded(#[serde(with = "WorldBackboneWireV1")] WorldBackboneDtoV1);

#[derive(Serialize)]
struct View<'a> {
    version: u16,
    #[serde(with = "records::character::map")]
    characters: &'a BTreeMap<ActorId, Character>,
    roster: &'a [ActorId],
    inventory: InventoryView<'a>,
    #[serde(with = "PlaceRegistryV1")]
    places: &'a PlaceRegistry,
    #[serde(with = "records::point::map")]
    household_doors: &'a BTreeMap<ActorId, Vec3>,
    #[serde(with = "records::command::map")]
    round_actions: &'a BTreeMap<ActorId, CommandId>,
    #[serde(with = "records::command::map")]
    travel_actions: &'a BTreeMap<ActorId, CommandId>,
    world_revision: i64,
    event_sequence: i64,
    spatial_sequence: i64,
    sounds_enabled: bool,
    view_cone_degrees: f64,
    #[serde(with = "records::world_time::option")]
    current_time: Option<WorldTime>,
    #[serde(with = "records::needle::option")]
    needle_claim: &'a Option<NeedleClaim>,
    spoke_this_turn: &'a Option<ActorId>,
}
impl<'a> View<'a> {
    fn new(w: &'a World) -> Self {
        Self {
            version: 1,
            characters: &w.characters,
            roster: &w.roster,
            inventory: InventoryView::new(w),
            places: &w.places,
            household_doors: &w.household_doors,
            round_actions: &w.round_actions,
            travel_actions: &w.travel_actions,
            world_revision: w.world_revision,
            event_sequence: w.event_sequence,
            spatial_sequence: w.spatial_sequence,
            sounds_enabled: w.sounds_enabled,
            view_cone_degrees: w.view_cone_degrees,
            current_time: w.current_time,
            needle_claim: &w.needle_claim,
            spoke_this_turn: &w.spoke_this_turn,
        }
    }
}
impl World {
    /// Diagnostic preflight only: requires and grows the attached save cohort
    /// before returning the cost. Does not construct a DTO or copy city records.
    pub fn checkpoint_backbone_cost(
        &self,
        mut reservation: Reservation,
    ) -> Result<Admitted<ComponentCost>> {
        let cost = aggregate::prepare_export(&View::new(self), OWNER, &mut reservation)?;
        Ok(Admitted::new(cost, reservation))
    }
    pub fn export_backbone_checkpoint(
        &self,
        mut reservation: Reservation,
    ) -> Result<Admitted<WorldBackboneDtoV1>> {
        check(self.events.is_empty(), "unflushed domain events")?;
        self.command_ledger.validate_checkpoint_boundary()?;
        aggregate::prepare_export(&View::new(self), OWNER, &mut reservation)?;
        let dto = WorldBackboneDtoV1 {
            version: 1,
            characters: self.characters.clone(),
            roster: self.roster.clone(),
            inventory: InventoryDtoV1::from_world(self),
            places: self.places.clone(),
            household_doors: (*self.household_doors).clone(),
            round_actions: self.round_actions.clone(),
            travel_actions: self.travel_actions.clone(),
            world_revision: self.world_revision,
            event_sequence: self.event_sequence,
            spatial_sequence: self.spatial_sequence,
            sounds_enabled: self.sounds_enabled,
            view_cone_degrees: self.view_cone_degrees,
            current_time: self.current_time,
            needle_claim: self.needle_claim.clone(),
            spoke_this_turn: self.spoke_this_turn.clone(),
        };
        dto.validate(&self.item_catalog, &self.command_ledger)?;
        Ok(Admitted::new(dto, reservation))
    }
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct BackboneCounts {
    pub characters: usize,
    pub items: usize,
    pub offers: usize,
    pub transforms: usize,
    pub completed_transforms: usize,
    pub places: usize,
}
impl WorldBackboneDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut reservation: Reservation,
        catalog: &ItemCatalog,
        ledger: &CommandLedger,
    ) -> Result<Admitted<Self>> {
        let Decoded(dto) = aggregate::decode(bytes, OWNER, &mut reservation)?;
        dto.validate(catalog, ledger)?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn cost(&self) -> Result<ComponentCost> {
        aggregate::measure(self, OWNER)
    }
    pub fn counts(&self) -> BackboneCounts {
        BackboneCounts {
            characters: self.characters.len(),
            items: self.inventory.items.len(),
            offers: self.inventory.offers.len(),
            transforms: self.inventory.transform_jobs.len(),
            completed_transforms: self.inventory.completed_transform_jobs.len(),
            places: self.places.checkpoint_entry_count(),
        }
    }
    pub fn validate(&self, catalog: &ItemCatalog, ledger: &CommandLedger) -> Result<()> {
        check(self.version == 1, "unsupported backbone version")?;
        check(
            self.characters.len() <= MAX_CHARACTERS && self.roster.len() == self.characters.len(),
            "roster count mismatch/limit",
        )?;
        let roster: BTreeSet<_> = self.roster.iter().collect();
        check(
            roster.len() == self.roster.len()
                && roster.iter().all(|id| self.characters.contains_key(*id)),
            "roster duplicates or missing actors",
        )?;
        for (id, c) in &self.characters {
            check(id == c.id(), "character key/sheet identity mismatch")?;
            crate::character::checkpoint::validate(c)?;
            check(
                c.state
                    .knows
                    .iter()
                    .all(|id| self.characters.contains_key(id)),
                "live perspective references unknown actor",
            )?;
            check(
                c.state
                    .places_known
                    .iter()
                    .all(|id| self.places.get(id).is_some()),
                "live perspective references unknown place",
            )?;
        }
        crate::places::checkpoint::validate(&self.places, &self.characters)?;
        for (id, p) in &self.household_doors {
            check(
                self.characters.contains_key(id),
                "household door has missing actor",
            )?;
            crate::character::checkpoint::point(*p)?;
        }
        check(
            (0..i64::MAX).contains(&self.world_revision)
                && (0..i64::MAX).contains(&self.event_sequence)
                && (-1..i64::MAX).contains(&self.spatial_sequence),
            "invalid/exhausted allocation counter",
        )?;
        check(
            self.view_cone_degrees.is_finite() && (0.0..=360.0).contains(&self.view_cone_degrees),
            "invalid view cone",
        )?;
        if let Some(t) = self.current_time {
            crate::checkpoint::calendar(OWNER, t.game_days())?;
            check(
                t.fraction.is_finite() && (0.0..1.0).contains(&t.fraction),
                "invalid sampled world time fraction",
            )?;
            check(
                t.office == WorldTime::from_game_days(t.fraction).office
                    && t.weekday == crate::clock::Weekday::of_day(t.day),
                "sampled world time labels disagree",
            )?;
        }
        if let Some(n) = &self.needle_claim {
            check(
                self.characters.contains_key(&n.holder),
                "needle holder missing",
            )?;
            crate::character::checkpoint::point(n.dir)?;
        }
        if let Some(id) = &self.spoke_this_turn {
            check(
                self.characters.contains_key(id),
                "turn speech marker actor missing",
            )?;
        }
        self.inventory
            .validate(&self.characters, catalog, self.event_sequence)?;
        check(
            self.round_actions.len() + self.travel_actions.len()
                <= crate::receipts::PROTECTED_CAPACITY,
            "too many legacy undertakings",
        )?;
        let mut ids = BTreeSet::new();
        for (round, map) in [(false, &self.travel_actions), (true, &self.round_actions)] {
            for (actor, id) in map {
                crate::character::checkpoint::command(*id)?;
                check(ids.insert(id), "duplicate legacy undertaking command")?;
                let c = self
                    .characters
                    .get(actor)
                    .ok_or_else(|| err("undertaking actor missing"))?;
                check(
                    c.state.presence == Presence::InCity,
                    "active undertaking actor absent",
                )?;
                let matching = if round {
                    c.state.round_edit.as_ref().is_some_and(|e| {
                        e.receipt == Some(*id) && e.presence_epoch == Some(c.state.presence_epoch)
                    })
                } else {
                    c.state
                        .intent
                        .as_ref()
                        .is_some_and(|i| i.receipt == Some(*id))
                };
                check(matching, "undertaking state binding mismatch")?;
                let receipt = ledger
                    .get(*id)
                    .ok_or_else(|| err("undertaking receipt missing"))?;
                check(
                    ledger.is_protected(id.operation)
                        && matches!(
                            receipt.outcome.state,
                            ReceiptState::Accepted | ReceiptState::InProgress
                        ),
                    "undertaking receipt/root mismatch",
                )?;
            }
        }
        // A cleared index may leave a terminal historical receipt in an old intent.
        // Only a retained nonterminal receipt demands live matching ownership.
        for (actor, c) in &self.characters {
            for (round, id) in [
                (false, c.state.intent.as_ref().and_then(|i| i.receipt)),
                (true, c.state.round_edit.as_ref().and_then(|e| e.receipt)),
            ] {
                if let Some(id) = id {
                    if ledger.get(id).is_some_and(|r| {
                        matches!(
                            r.outcome.state,
                            ReceiptState::Accepted | ReceiptState::InProgress
                        )
                    }) {
                        check(
                            (if round {
                                &self.round_actions
                            } else {
                                &self.travel_actions
                            })
                            .get(actor)
                                == Some(&id),
                            "nonterminal undertaking has no live binding",
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}
/// Fully validated covered authority, still incapable of replacing World.
#[derive(Debug)]
pub struct BackboneCandidate {
    data: WorldBackboneDtoV1,
}
impl BackboneCandidate {
    pub fn counts(&self) -> BackboneCounts {
        self.data.counts()
    }
}
impl Admitted<WorldBackboneDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|dto, r| aggregate::encode(&dto, OWNER, r))
    }
    /// Ownership transfer into covered candidate maps; typed decoding already
    /// constructs the BTree/PlaceRegistry indexes. This phase revalidates them.
    pub fn into_candidate(
        self,
        catalog: &ItemCatalog,
        ledger: &CommandLedger,
    ) -> Result<Admitted<BackboneCandidate>> {
        self.try_map(|data, _| {
            data.validate(catalog, ledger)?;
            Ok(BackboneCandidate { data })
        })
    }
}
impl Admitted<BackboneCandidate> {
    #[cfg(test)]
    pub(crate) fn install_for_test(&self, w: &mut World) {
        // Test witness only: the caller retains the admitted candidate through all
        // comparisons. There is deliberately no production World replacement API.
        let d = &self.value().data;
        w.characters = d.characters.clone();
        w.roster = d.roster.clone();
        w.items = d.inventory.items.clone();
        w.offers = d.inventory.offers.clone();
        w.legacy_restock_shares = d.inventory.legacy_restock_shares.clone();
        w.transform_jobs = d.inventory.transform_jobs.clone();
        w.completed_transform_jobs = d.inventory.completed_transform_jobs.clone();
        w.places = d.places.clone();
        w.household_doors = Arc::new(d.household_doors.clone());
        w.round_actions = d.round_actions.clone();
        w.travel_actions = d.travel_actions.clone();
        w.world_revision = d.world_revision;
        w.event_sequence = d.event_sequence;
        w.spatial_sequence = d.spatial_sequence;
        w.sounds_enabled = d.sounds_enabled;
        w.view_cone_degrees = d.view_cone_degrees;
        w.current_time = d.current_time;
        w.needle_claim = d.needle_claim.clone();
        w.spoke_this_turn = d.spoke_this_turn.clone();
    }
}
fn err(reason: &'static str) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn check(ok: bool, reason: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(err(reason)) }
}

#[cfg(test)]
mod tests;

/// Read-only covered authority shared by running export and unadopted candidate
/// validation. No World construction, seeding, replacement or clone is needed.
#[derive(Clone, Copy)]
pub(crate) struct BackboneRefs<'a> {
    pub current_time: Option<WorldTime>,
    pub sounds_enabled: bool,
    pub characters: &'a BTreeMap<ActorId, Character>,
    pub items: &'a BTreeMap<ItemId, Item>,
    pub places: &'a PlaceRegistry,
    pub household_doors: &'a BTreeMap<ActorId, Vec3>,
    pub shares: &'a BTreeMap<ItemId, Vec<crate::inventory::LegacyRestockShare>>,
}
impl<'a> BackboneRefs<'a> {
    pub(crate) fn from_world(w: &'a World) -> Self {
        Self {
            current_time: w.current_time,
            sounds_enabled: w.sounds_enabled,
            characters: &w.characters,
            items: &w.items,
            places: &w.places,
            household_doors: &w.household_doors,
            shares: &w.legacy_restock_shares,
        }
    }
}
impl BackboneCandidate {
    pub(crate) fn references(&self) -> BackboneRefs<'_> {
        let d = &self.data;
        BackboneRefs {
            current_time: d.current_time,
            sounds_enabled: d.sounds_enabled,
            characters: &d.characters,
            items: &d.inventory.items,
            places: &d.places,
            household_doors: &d.household_doors,
            shares: &d.inventory.legacy_restock_shares,
        }
    }
}
