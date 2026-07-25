//! Central inventory authority for the M5 supply chain.
//!
//! Before M5 the action verbs and the food round each edited `holds`, `items`
//! and purses independently.  That is no longer safe once a quantity may be
//! promised to an offer, reserved by a transform, or marked as legacy conjured
//! stock.  Every quantity-changing supply-chain path is therefore routed
//! through the methods in this module.

use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{
    event::DomainEvent,
    ids::{ActorId, ItemId},
    item::{Item, ItemKind},
    world::{World, mint_item_id},
};

/// Exact inventory identity. Metadata matching is deliberately whole-map
/// equality: there are no wildcard or subset matchers in the supply chain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemMatcher {
    pub kind: ItemKind,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ItemMatcher {
    pub fn new(kind: impl Into<ItemKind>) -> Self {
        Self {
            kind: kind.into(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn matches(&self, item: &Item) -> bool {
        self.kind == item.kind && self.metadata == item.metadata
    }

    pub fn to_item(&self, id: ItemId, quantity: u32) -> Item {
        Item {
            id,
            kind: self.kind.clone(),
            quantity,
            metadata: self.metadata.clone(),
        }
    }
}

/// A positive quantity of exact stock, used by manifests and recipes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockSpec {
    pub kind: ItemKind,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub quantity: u32,
}

impl StockSpec {
    pub fn matcher(&self) -> ItemMatcher {
        ItemMatcher {
            kind: self.kind.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Quantity in a normal held stack which belongs to a magic legacy restock.
/// Provenance is operational state and never travels with transferred goods.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyRestockShare {
    pub original_vendor: ActorId,
    pub source_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservedInput {
    pub item_id: ItemId,
    pub quantity: u32,
}

/// One resumable producer job. Jobs live in `World` because an action verb must
/// see their commitments even though the round owns the production planner.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformJob {
    pub job_id: String,
    pub spec_id: String,
    pub producer: ActorId,
    pub production_day: i64,
    pub start_slot: u32,
    pub inputs: Vec<ReservedInput>,
    pub outputs: Vec<StockSpec>,
    pub progress_work_minutes: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformReceiptLine {
    pub item_id: ItemId,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformReceipt {
    pub job_id: String,
    pub producer: ActorId,
    pub consumed: Vec<TransformReceiptLine>,
    pub produced: Vec<TransformReceiptLine>,
    pub completed_on_day: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedTransform {
    pub receipt: TransformReceipt,
    pub completed_on_day: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketRequestLine {
    pub matcher: ItemMatcher,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaleReceiptLine {
    pub source_item_id: ItemId,
    pub destination_item_id: ItemId,
    pub matcher: ItemMatcher,
    pub quantity: u32,
    pub unit_price_sparks: u32,
    pub line_total_sparks: u32,
}

/// A purpose-neutral catalog-price transaction. Meal and stock planners both
/// receive this same receipt; no intent marker is stored on it or the goods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaleReceipt {
    pub operation_key: String,
    pub buyer: ActorId,
    pub seller: ActorId,
    pub lines: Vec<SaleReceiptLine>,
    pub total_sparks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InventoryErrorCode {
    UnknownActor,
    UnknownItem,
    NotOwner,
    BadQuantity,
    ItemCommitted,
    OutputCapacityReserved,
    ArithmeticOverflow,
    DuplicateTransform,
    NoActiveTransformJob,
    NoMatchingStock,
    UnpricedStock,
    InsufficientFunds,
    BudgetExhausted,
    SelfSale,
    InvalidContent,
}

impl InventoryErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownActor => "unknown_actor",
            Self::UnknownItem => "unknown_item",
            Self::NotOwner => "not_owner",
            Self::BadQuantity => "bad_quantity",
            Self::ItemCommitted => "item_committed",
            Self::OutputCapacityReserved => "output_capacity_reserved",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::DuplicateTransform => "duplicate_transform",
            Self::NoActiveTransformJob => "no_active_transform_job",
            Self::NoMatchingStock => "no_matching_stock",
            Self::UnpricedStock => "unpriced_stock",
            Self::InsufficientFunds => "insufficient_funds",
            Self::BudgetExhausted => "budget_exhausted",
            Self::SelfSale => "self_sale",
            Self::InvalidContent => "invalid_content",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryError {
    pub code: InventoryErrorCode,
    pub message: String,
}

impl InventoryError {
    pub fn new(code: InventoryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for InventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InventoryError {}

impl World {
    /// The unique holder of an item, if it is held at all.
    pub fn owner_of(&self, item_id: &ItemId) -> Option<&ActorId> {
        self.characters
            .iter()
            .find_map(|(id, character)| character.holds().contains(item_id).then_some(id))
    }

    pub fn matcher_of(&self, item_id: &ItemId) -> Option<ItemMatcher> {
        self.items.get(item_id).map(|item| ItemMatcher {
            kind: item.kind.clone(),
            metadata: item.metadata.clone(),
        })
    }

    /// Quantity promised by the live offer on this stack.
    pub fn offered_quantity(&self, item_id: &ItemId) -> u32 {
        self.offers.get(item_id).map_or(0, |offer| offer.quantity)
    }

    /// Quantity reserved by all active transforms on this stack.
    pub fn transform_reserved_quantity(&self, item_id: &ItemId) -> u32 {
        self.transform_jobs
            .values()
            .flat_map(|job| &job.inputs)
            .filter(|reserved| &reserved.item_id == item_id)
            .try_fold(0u32, |sum, reserved| sum.checked_add(reserved.quantity))
            .expect("transform reservations overflow an item stack")
    }

    /// Quantity of this stack riding in the owner's body pockets
    /// (`features/extra_pockets.md`): a commitment exactly like an offer —
    /// retrieval-first is the rule, so a pocketed unit cannot be offered, sold,
    /// eaten or counted by a restock while it rides there.
    pub fn pocketed_quantity(&self, item_id: &ItemId) -> u32 {
        self.characters
            .values()
            .flat_map(|character| character.pockets())
            .filter(|unit| &unit.item_id == item_id)
            .count() as u32
    }

    /// Quantity available to a new operation. Offers, transforms and body
    /// pockets are all commitments; none silently loses its promise to a later
    /// caller.
    pub fn uncommitted_quantity(&self, item_id: &ItemId) -> u32 {
        let held = self.items.get(item_id).map_or(0, |item| item.quantity);
        held.saturating_sub(
            self.offered_quantity(item_id)
                .saturating_add(self.transform_reserved_quantity(item_id))
                .saturating_add(self.pocketed_quantity(item_id)),
        )
    }

    /// Offer replacement may provisionally release precisely the offer being
    /// replaced, while retaining every transform and pocket reservation.
    pub fn available_for_offer_replacement(&self, item_id: &ItemId) -> u32 {
        let held = self.items.get(item_id).map_or(0, |item| item.quantity);
        held.saturating_sub(
            self.transform_reserved_quantity(item_id)
                .saturating_add(self.pocketed_quantity(item_id)),
        )
    }

    pub fn held_quantity(&self, owner: &ActorId, matcher: &ItemMatcher) -> u32 {
        self.characters
            .get(owner)
            .map(|character| {
                character
                    .holds()
                    .iter()
                    .filter_map(|id| self.items.get(id))
                    .filter(|item| matcher.matches(item))
                    .fold(0u32, |sum, item| sum.saturating_add(item.quantity))
            })
            .unwrap_or(0)
    }

    pub fn uncommitted_held_quantity(&self, owner: &ActorId, matcher: &ItemMatcher) -> u32 {
        self.characters
            .get(owner)
            .map(|character| {
                character
                    .holds()
                    .iter()
                    .filter(|id| {
                        self.items
                            .get(*id)
                            .is_some_and(|item| matcher.matches(item))
                    })
                    .fold(0u32, |sum, id| {
                        sum.saturating_add(self.uncommitted_quantity(id))
                    })
            })
            .unwrap_or(0)
    }

    pub fn spendable_sparks(&self, owner: &ActorId) -> u32 {
        self.uncommitted_held_quantity(owner, &ItemMatcher::new("spark"))
    }

    pub fn wallet_sparks(&self, owner: &ActorId) -> u32 {
        self.held_quantity(owner, &ItemMatcher::new("spark"))
    }

    /// Set a purse to an exact boundary float without ever crediting an item id
    /// that has since moved to another owner. Returns `(cash_in, cash_out)`.
    pub fn settle_wallet_exact(
        &mut self,
        owner: &ActorId,
        target: u32,
        operation_key: &str,
    ) -> Result<(u32, u32), InventoryError> {
        if !self.characters.contains_key(owner) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                format!("unknown wallet owner '{owner}'"),
            ));
        }
        let current = self.wallet_sparks(owner);
        if self.spendable_sparks(owner) != current {
            return Err(InventoryError::new(
                InventoryErrorCode::ItemCommitted,
                format!("{owner}'s purse is committed to a pending offer"),
            ));
        }
        match current.cmp(&target) {
            std::cmp::Ordering::Less => {
                let amount = target - current;
                self.credit_sparks(owner, amount, operation_key)?;
                Ok((amount, 0))
            }
            std::cmp::Ordering::Greater => {
                let amount = current - target;
                self.debit_sparks(owner, amount)?;
                Ok((0, amount))
            }
            std::cmp::Ordering::Equal => Ok((0, 0)),
        }
    }

    /// Future output already promised for this owner's exact stacking key.
    pub fn future_output_quantity(&self, owner: &ActorId, matcher: &ItemMatcher) -> u32 {
        self.transform_jobs
            .values()
            .filter(|job| &job.producer == owner)
            .flat_map(|job| &job.outputs)
            .filter(|stock| matcher == &stock.matcher())
            .try_fold(0u32, |sum, stock| sum.checked_add(stock.quantity))
            .expect("future transform outputs overflow their stacking key")
    }

    /// Add or merge ordinary stock, respecting active future-output capacity.
    /// The operation key determines a stable collision-probed id when no merge
    /// target exists. This method does not bump `world_revision`; callers batch
    /// related writes and touch once.
    pub fn add_stock(
        &mut self,
        owner: &ActorId,
        stock: &StockSpec,
        operation_key: &str,
    ) -> Result<ItemId, InventoryError> {
        if stock.quantity == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::BadQuantity,
                "stock quantity must be positive",
            ));
        }
        if !self.characters.contains_key(owner) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                format!("unknown stock owner '{owner}'"),
            ));
        }
        let matcher = stock.matcher();
        let probe = matcher.to_item(ItemId::from_raw("inventory_probe"), stock.quantity);
        self.item_catalog
            .validate_seed_item(&probe)
            .map_err(|message| InventoryError::new(InventoryErrorCode::InvalidContent, message))?;
        let stackable = self.item_catalog.stackable(&probe);
        let held = self.held_quantity(owner, &matcher);
        let future = self.future_output_quantity(owner, &matcher);
        held.checked_add(future)
            .and_then(|sum| sum.checked_add(stock.quantity))
            .ok_or_else(|| {
                let code = if future > 0 {
                    InventoryErrorCode::OutputCapacityReserved
                } else {
                    InventoryErrorCode::ArithmeticOverflow
                };
                InventoryError::new(
                    code,
                    format!("adding stock to {owner} would overflow its stacking key"),
                )
            })?;

        let existing = stackable
            .then(|| {
                self.characters[owner]
                    .holds()
                    .iter()
                    .find(|id| {
                        self.items
                            .get(*id)
                            .is_some_and(|item| matcher.matches(item))
                    })
                    .cloned()
            })
            .flatten();
        if let Some(id) = existing {
            let item = self.items.get_mut(&id).expect("held stack exists");
            item.quantity = item.quantity.checked_add(stock.quantity).ok_or_else(|| {
                InventoryError::new(InventoryErrorCode::ArithmeticOverflow, "stack overflow")
            })?;
            return Ok(id);
        }

        let id = self.resolve_item_id(&ItemId::from_raw(owner.as_str()), operation_key);
        let item = matcher.to_item(id.clone(), stock.quantity);
        self.items.insert(id.clone(), item);
        self.characters
            .get_mut(owner)
            .expect("owner checked")
            .state
            .holds
            .push(id.clone());
        Ok(id)
    }

    /// Transfer uncommitted quantity. `released_offer` is used only by offer
    /// acceptance, allowing that offer's own promise while retaining all other
    /// commitments.
    pub fn transfer_item_quantity(
        &mut self,
        from: &ActorId,
        to: &ActorId,
        item_id: &ItemId,
        quantity: u32,
        operation_key: &str,
    ) -> Result<ItemId, InventoryError> {
        self.transfer_item_quantity_releasing(from, to, item_id, quantity, operation_key, false)
    }

    pub fn transfer_offered_item(
        &mut self,
        from: &ActorId,
        to: &ActorId,
        item_id: &ItemId,
        quantity: u32,
        operation_key: &str,
    ) -> Result<ItemId, InventoryError> {
        let valid_offer = self
            .offers
            .get(item_id)
            .is_some_and(|offer| offer.giver_id == *from && offer.quantity == quantity);
        if !valid_offer {
            return Err(InventoryError::new(
                InventoryErrorCode::ItemCommitted,
                "offer acceptance must release exactly the live offer's commitment",
            ));
        }
        self.transfer_item_quantity_releasing(from, to, item_id, quantity, operation_key, true)
    }

    fn transfer_item_quantity_releasing(
        &mut self,
        from: &ActorId,
        to: &ActorId,
        item_id: &ItemId,
        quantity: u32,
        operation_key: &str,
        release_offer: bool,
    ) -> Result<ItemId, InventoryError> {
        if from == to {
            return Err(InventoryError::new(
                InventoryErrorCode::InvalidContent,
                "an inventory transfer needs two different owners",
            ));
        }
        if quantity == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::BadQuantity,
                "transfer quantity must be positive",
            ));
        }
        if !self.characters.contains_key(from) || !self.characters.contains_key(to) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                "transfer actor is not in the world",
            ));
        }
        if !self.characters[from].holds().contains(item_id) {
            return Err(InventoryError::new(
                InventoryErrorCode::NotOwner,
                format!("{from} does not hold {item_id}"),
            ));
        }
        let item = self.items.get(item_id).cloned().ok_or_else(|| {
            InventoryError::new(
                InventoryErrorCode::UnknownItem,
                format!("missing item {item_id}"),
            )
        })?;
        let released = if release_offer {
            self.offers
                .get(item_id)
                .filter(|offer| offer.giver_id == *from)
                .map_or(0, |offer| offer.quantity)
        } else {
            0
        };
        let available = self.uncommitted_quantity(item_id).saturating_add(released);
        if quantity > available {
            return Err(InventoryError::new(
                InventoryErrorCode::ItemCommitted,
                format!(
                    "only {available} of {item_id} is uncommitted; retract or replace its offer first"
                ),
            ));
        }

        let matcher = ItemMatcher {
            kind: item.kind.clone(),
            metadata: item.metadata.clone(),
        };
        let merge_target = if self.item_catalog.stackable(&item) {
            self.characters[to]
                .holds()
                .iter()
                .find(|id| {
                    self.items
                        .get(*id)
                        .is_some_and(|other| matcher.matches(other))
                })
                .cloned()
        } else {
            None
        };
        let destination_held = self.held_quantity(to, &matcher);
        let destination_future = self.future_output_quantity(to, &matcher);
        destination_held
            .checked_add(destination_future)
            .and_then(|sum| sum.checked_add(quantity))
            .ok_or_else(|| {
                let code = if destination_future > 0 {
                    InventoryErrorCode::OutputCapacityReserved
                } else {
                    InventoryErrorCode::ArithmeticOverflow
                };
                InventoryError::new(
                    code,
                    format!("the transfer would overflow {to}'s stacking key"),
                )
            })?;
        if let Some(target) = &merge_target {
            self.items[target]
                .quantity
                .checked_add(quantity)
                .ok_or_else(|| {
                    InventoryError::new(InventoryErrorCode::ArithmeticOverflow, "stack overflow")
                })?;
        }

        // Provenance is consumed before unmarked quantity and never follows a
        // transfer, even when the whole stack keeps its public item id.
        self.deduct_legacy_shares(item_id, quantity);
        let whole = quantity == item.quantity;
        let destination = if whole {
            self.characters
                .get_mut(from)
                .expect("source checked")
                .state
                .holds
                .retain(|held| held != item_id);
            match merge_target {
                Some(target) => {
                    self.items
                        .get_mut(&target)
                        .expect("merge target exists")
                        .quantity += quantity;
                    self.items.remove(item_id);
                    self.legacy_restock_shares.remove(item_id);
                    target
                }
                None => {
                    self.characters
                        .get_mut(to)
                        .expect("destination checked")
                        .state
                        .holds
                        .push(item_id.clone());
                    self.legacy_restock_shares.remove(item_id);
                    item_id.clone()
                }
            }
        } else {
            self.items.get_mut(item_id).expect("source exists").quantity -= quantity;
            match merge_target {
                Some(target) => {
                    self.items
                        .get_mut(&target)
                        .expect("merge target exists")
                        .quantity += quantity;
                    target
                }
                None => {
                    let new_id = self.resolve_item_id(item_id, operation_key);
                    let mut moved = item;
                    moved.id = new_id.clone();
                    moved.quantity = quantity;
                    self.items.insert(new_id.clone(), moved);
                    self.characters
                        .get_mut(to)
                        .expect("destination checked")
                        .state
                        .holds
                        .push(new_id.clone());
                    new_id
                }
            }
        };
        Ok(destination)
    }

    /// Consume uncommitted quantity in place (eating, boundary unload, legacy
    /// cleanup). Active offers and transforms are never silently shortened.
    pub fn consume_item_quantity(
        &mut self,
        owner: &ActorId,
        item_id: &ItemId,
        quantity: u32,
    ) -> Result<(), InventoryError> {
        self.consume_item_quantity_for_job(owner, item_id, quantity, None)
    }

    fn consume_item_quantity_for_job(
        &mut self,
        owner: &ActorId,
        item_id: &ItemId,
        quantity: u32,
        job_id: Option<&str>,
    ) -> Result<(), InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::BadQuantity,
                "consume quantity must be positive",
            ));
        }
        if !self
            .characters
            .get(owner)
            .is_some_and(|actor| actor.holds().contains(item_id))
        {
            return Err(InventoryError::new(
                InventoryErrorCode::NotOwner,
                format!("{owner} does not hold {item_id}"),
            ));
        }
        let held = self.items.get(item_id).map_or(0, |item| item.quantity);
        let own_reservation = job_id.map_or(0, |wanted| {
            self.transform_jobs
                .values()
                .filter(|job| job.job_id == wanted)
                .flat_map(|job| &job.inputs)
                .filter(|input| &input.item_id == item_id)
                .fold(0u32, |sum, input| sum.saturating_add(input.quantity))
        });
        let available = self
            .uncommitted_quantity(item_id)
            .saturating_add(own_reservation);
        if quantity > available {
            return Err(InventoryError::new(
                InventoryErrorCode::ItemCommitted,
                format!("only {available} of {item_id} is available"),
            ));
        }
        self.deduct_legacy_shares(item_id, quantity);
        if quantity == held {
            self.characters
                .get_mut(owner)
                .expect("owner checked")
                .state
                .holds
                .retain(|held_id| held_id != item_id);
            self.items.remove(item_id);
            self.legacy_restock_shares.remove(item_id);
        } else {
            self.items.get_mut(item_id).expect("item checked").quantity -= quantity;
        }
        Ok(())
    }

    /// Re-stamp `quantity` uncommitted units of `item_id` with metadata
    /// `key=value`, keeping them with the same owner (`extra_pockets.md` M2 —
    /// the wet coin, the poopstained bread). Metadata is stack identity, so
    /// this forks the stack: the restamped units leave for (or merge into) a
    /// stack differing only in `key`. Returns the id of the stack now carrying
    /// the restamped units. Does not bump `world_revision`; callers batch.
    pub fn restamp_metadata(
        &mut self,
        owner: &ActorId,
        item_id: &ItemId,
        quantity: u32,
        key: &str,
        value: &str,
        operation_key: &str,
    ) -> Result<ItemId, InventoryError> {
        if quantity == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::BadQuantity,
                "restamp quantity must be positive",
            ));
        }
        if !self
            .characters
            .get(owner)
            .is_some_and(|actor| actor.holds().contains(item_id))
        {
            return Err(InventoryError::new(
                InventoryErrorCode::NotOwner,
                format!("{owner} does not hold {item_id}"),
            ));
        }
        let item = self.items.get(item_id).cloned().ok_or_else(|| {
            InventoryError::new(
                InventoryErrorCode::UnknownItem,
                format!("missing item {item_id}"),
            )
        })?;
        if item.metadata.get(key).map(String::as_str) == Some(value) {
            return Ok(item_id.clone());
        }
        let available = self.uncommitted_quantity(item_id);
        if quantity > available {
            return Err(InventoryError::new(
                InventoryErrorCode::ItemCommitted,
                format!("only {available} of {item_id} is uncommitted"),
            ));
        }
        let mut metadata = item.metadata.clone();
        metadata.insert(key.to_string(), value.to_string());
        let matcher = ItemMatcher {
            kind: item.kind.clone(),
            metadata,
        };
        let probe = matcher.to_item(ItemId::from_raw("restamp_probe"), quantity);
        self.item_catalog
            .validate_item(&probe)
            .map_err(|message| InventoryError::new(InventoryErrorCode::InvalidContent, message))?;
        let merge_target = if self.item_catalog.stackable(&item) {
            self.characters[owner]
                .holds()
                .iter()
                .find(|id| {
                    self.items
                        .get(*id)
                        .is_some_and(|other| matcher.matches(other))
                })
                .cloned()
        } else {
            None
        };
        if let Some(target) = &merge_target {
            self.items[target]
                .quantity
                .checked_add(quantity)
                .ok_or_else(|| {
                    InventoryError::new(InventoryErrorCode::ArithmeticOverflow, "stack overflow")
                })?;
        }

        // Provenance never follows a fork, exactly like a transfer.
        self.deduct_legacy_shares(item_id, quantity);
        let whole = quantity == item.quantity;
        let destination = match (whole, merge_target) {
            (true, Some(target)) => {
                self.items
                    .get_mut(&target)
                    .expect("merge target exists")
                    .quantity += quantity;
                self.items.remove(item_id);
                self.legacy_restock_shares.remove(item_id);
                self.characters
                    .get_mut(owner)
                    .expect("owner checked")
                    .state
                    .holds
                    .retain(|held| held != item_id);
                target
            }
            (true, None) => {
                self.items
                    .get_mut(item_id)
                    .expect("item checked")
                    .metadata
                    .insert(key.to_string(), value.to_string());
                item_id.clone()
            }
            (false, Some(target)) => {
                self.items.get_mut(item_id).expect("item checked").quantity -= quantity;
                self.items
                    .get_mut(&target)
                    .expect("merge target exists")
                    .quantity += quantity;
                target
            }
            (false, None) => {
                self.items.get_mut(item_id).expect("item checked").quantity -= quantity;
                let new_id = self.resolve_item_id(item_id, operation_key);
                let forked = matcher.to_item(new_id.clone(), quantity);
                self.items.insert(new_id.clone(), forked);
                self.characters
                    .get_mut(owner)
                    .expect("owner checked")
                    .state
                    .holds
                    .push(new_id.clone());
                new_id
            }
        };
        Ok(destination)
    }

    /// Add a legacy template and mark only the newly conjured quantity.
    pub fn add_legacy_restock(
        &mut self,
        owner: &ActorId,
        source_id: &str,
        stock: &StockSpec,
        operation_key: &str,
    ) -> Result<ItemId, InventoryError> {
        let matcher = stock.matcher();
        let existing_id = self.characters.get(owner).and_then(|character| {
            character.holds().iter().find(|id| {
                self.items
                    .get(*id)
                    .is_some_and(|item| matcher.matches(item))
            })
        });
        let existing_share = existing_id
            .and_then(|id| self.legacy_restock_shares.get(id))
            .and_then(|shares| {
                shares
                    .iter()
                    .find(|share| share.original_vendor == *owner && share.source_id == source_id)
            })
            .map_or(0, |share| share.quantity);
        existing_share.checked_add(stock.quantity).ok_or_else(|| {
            InventoryError::new(
                InventoryErrorCode::ArithmeticOverflow,
                "restock share overflow",
            )
        })?;
        let id = self.add_stock(owner, stock, operation_key)?;
        let shares = self.legacy_restock_shares.entry(id.clone()).or_default();
        if let Some(existing) = shares
            .iter_mut()
            .find(|share| share.original_vendor == *owner && share.source_id == source_id)
        {
            existing.quantity += stock.quantity;
        } else {
            shares.push(LegacyRestockShare {
                original_vendor: owner.clone(),
                source_id: source_id.to_string(),
                quantity: stock.quantity,
            });
            shares.sort();
        }
        Ok(id)
    }

    /// Remove the uncommitted portion of one legacy source while preserving
    /// real returned stock and commitments. Returns the quantity swept.
    pub fn sweep_legacy_restock(&mut self, source_id: &str) -> Result<u32, InventoryError> {
        let mut staged = self.clone();
        let swept = staged.sweep_legacy_restock_inner(source_id)?;
        *self = staged;
        Ok(swept)
    }

    fn sweep_legacy_restock_inner(&mut self, source_id: &str) -> Result<u32, InventoryError> {
        let ids: Vec<ItemId> = self.legacy_restock_shares.keys().cloned().collect();
        let mut swept = 0u32;
        for id in ids {
            let Some(index) = self
                .legacy_restock_shares
                .get(&id)
                .and_then(|shares| shares.iter().position(|share| share.source_id == source_id))
            else {
                continue;
            };
            let share = self.legacy_restock_shares[&id][index].clone();
            if self.owner_of(&id) != Some(&share.original_vendor) {
                return Err(InventoryError::new(
                    InventoryErrorCode::InvalidContent,
                    format!("legacy share {source_id} on {id} left its original vendor"),
                ));
            }
            let remove = share.quantity.min(self.uncommitted_quantity(&id));
            if remove == 0 {
                continue;
            }
            {
                let shares = self
                    .legacy_restock_shares
                    .get_mut(&id)
                    .expect("share exists");
                shares[index].quantity -= remove;
                shares.retain(|share| share.quantity > 0);
            }
            let held = self.items[&id].quantity;
            if remove == held {
                self.characters
                    .get_mut(&share.original_vendor)
                    .expect("owner exists")
                    .state
                    .holds
                    .retain(|held_id| held_id != &id);
                self.items.remove(&id);
                self.legacy_restock_shares.remove(&id);
            } else {
                self.items.get_mut(&id).expect("item exists").quantity -= remove;
                if self
                    .legacy_restock_shares
                    .get(&id)
                    .is_some_and(Vec::is_empty)
                {
                    self.legacy_restock_shares.remove(&id);
                }
            }
            swept = swept.checked_add(remove).ok_or_else(|| {
                InventoryError::new(
                    InventoryErrorCode::ArithmeticOverflow,
                    "sweep total overflow",
                )
            })?;
        }
        Ok(swept)
    }

    pub fn legacy_restock_shares(&self, item_id: &ItemId) -> &[LegacyRestockShare] {
        self.legacy_restock_shares
            .get(item_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Reserve a fully specified transform job atomically.
    pub fn start_transform_job(&mut self, job: TransformJob) -> Result<(), InventoryError> {
        if self.transform_jobs.contains_key(&job.producer) {
            return Err(InventoryError::new(
                InventoryErrorCode::DuplicateTransform,
                format!("{} already has an active transform", job.producer),
            ));
        }
        if !self.characters.contains_key(&job.producer) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                format!("unknown producer '{}'", job.producer),
            ));
        }
        if job.job_id.is_empty()
            || job.spec_id.is_empty()
            || job.inputs.is_empty()
            || job.outputs.is_empty()
            || !job.progress_work_minutes.is_finite()
            || job.progress_work_minutes < 0.0
        {
            return Err(InventoryError::new(
                InventoryErrorCode::InvalidContent,
                "transform jobs need ids, inputs, outputs, and finite non-negative progress",
            ));
        }
        if self.completed_transform_jobs.contains_key(&job.job_id) {
            return Err(InventoryError::new(
                InventoryErrorCode::DuplicateTransform,
                format!("transform job '{}' has already completed", job.job_id),
            ));
        }
        let mut required: BTreeMap<&ItemId, u32> = BTreeMap::new();
        for input in &job.inputs {
            if input.quantity == 0 {
                return Err(InventoryError::new(
                    InventoryErrorCode::BadQuantity,
                    "reserved input must be positive",
                ));
            }
            let amount = required.entry(&input.item_id).or_default();
            *amount = amount.checked_add(input.quantity).ok_or_else(|| {
                InventoryError::new(
                    InventoryErrorCode::ArithmeticOverflow,
                    "reservation overflow",
                )
            })?;
        }
        for (item_id, quantity) in required {
            if self.owner_of(item_id) != Some(&job.producer) {
                return Err(InventoryError::new(
                    InventoryErrorCode::NotOwner,
                    format!("producer does not hold reserved input {item_id}"),
                ));
            }
            if quantity > self.uncommitted_quantity(item_id) {
                return Err(InventoryError::new(
                    InventoryErrorCode::ItemCommitted,
                    format!("input {item_id} is already committed"),
                ));
            }
        }
        let mut new_outputs: BTreeMap<ItemMatcher, u32> = BTreeMap::new();
        for output in &job.outputs {
            if output.quantity == 0 {
                return Err(InventoryError::new(
                    InventoryErrorCode::BadQuantity,
                    "transform output must be positive",
                ));
            }
            let quantity = new_outputs.entry(output.matcher()).or_default();
            *quantity = quantity.checked_add(output.quantity).ok_or_else(|| {
                InventoryError::new(InventoryErrorCode::ArithmeticOverflow, "output overflow")
            })?;
            let probe = output
                .matcher()
                .to_item(ItemId::from_raw("transform_probe"), output.quantity);
            self.item_catalog
                .validate_seed_item(&probe)
                .map_err(|message| {
                    InventoryError::new(InventoryErrorCode::InvalidContent, message)
                })?;
        }
        for (matcher, added) in new_outputs {
            self.held_quantity(&job.producer, &matcher)
                .checked_add(self.future_output_quantity(&job.producer, &matcher))
                .and_then(|sum| sum.checked_add(added))
                .ok_or_else(|| {
                    InventoryError::new(
                        InventoryErrorCode::OutputCapacityReserved,
                        "future transform output would overflow its stack",
                    )
                })?;
        }
        self.transform_jobs.insert(job.producer.clone(), job);
        Ok(())
    }

    pub fn active_transform_job(&self, producer: &ActorId) -> Option<&TransformJob> {
        self.transform_jobs.get(producer)
    }

    pub fn active_transform_job_mut(&mut self, producer: &ActorId) -> Option<&mut TransformJob> {
        self.transform_jobs.get_mut(producer)
    }

    pub fn transform_jobs(&self) -> impl Iterator<Item = &TransformJob> {
        self.transform_jobs.values()
    }

    /// Finish the producer's active job. Callers that need idempotent replay by
    /// logical job key use [`Self::complete_transform_job_by_id`].
    pub fn complete_transform_job(
        &mut self,
        producer: &ActorId,
        current_day: i64,
    ) -> Result<TransformReceipt, InventoryError> {
        let Some(job_id) = self
            .transform_jobs
            .get(producer)
            .map(|job| job.job_id.clone())
        else {
            return Err(InventoryError::new(
                InventoryErrorCode::NoActiveTransformJob,
                format!("{producer} has no active transform job"),
            ));
        };
        self.complete_transform_job_by_id(producer, &job_id, current_day)
    }

    /// Finish exactly once by the full logical job key. Replays inside the
    /// current-and-previous-day receipt window return the original receipt
    /// without consuming or producing anything again. The clone makes even a
    /// late content/capacity failure an atomic no-op.
    pub fn complete_transform_job_by_id(
        &mut self,
        producer: &ActorId,
        job_id: &str,
        current_day: i64,
    ) -> Result<TransformReceipt, InventoryError> {
        let mut staged = self.clone();
        staged.prune_completed_transforms(current_day);
        let receipt = staged.complete_transform_job_inner(producer, job_id, current_day)?;
        *self = staged;
        Ok(receipt)
    }

    fn complete_transform_job_inner(
        &mut self,
        producer: &ActorId,
        job_id: &str,
        current_day: i64,
    ) -> Result<TransformReceipt, InventoryError> {
        if let Some(completed) = self.completed_transform_jobs.get(job_id) {
            if &completed.receipt.producer == producer {
                return Ok(completed.receipt.clone());
            }
            return Err(InventoryError::new(
                InventoryErrorCode::InvalidContent,
                format!("transform job '{job_id}' belongs to another producer"),
            ));
        }
        let Some(job) = self.transform_jobs.get(producer).cloned() else {
            return Err(InventoryError::new(
                InventoryErrorCode::NoActiveTransformJob,
                format!("{producer} has no active transform job '{job_id}'"),
            ));
        };
        if job.job_id != job_id {
            return Err(InventoryError::new(
                InventoryErrorCode::NoActiveTransformJob,
                format!("{producer}'s active transform is not '{job_id}'"),
            ));
        }

        // Every fallible capacity/ownership check happens before mutation.
        for input in &job.inputs {
            if self.owner_of(&input.item_id) != Some(producer)
                || self
                    .items
                    .get(&input.item_id)
                    .map_or(0, |item| item.quantity)
                    < input.quantity
            {
                return Err(InventoryError::new(
                    InventoryErrorCode::InvalidContent,
                    format!(
                        "reserved transform input {} is no longer intact",
                        input.item_id
                    ),
                ));
            }
        }
        let mut produced_keys = BTreeSet::new();
        for output in &job.outputs {
            let matcher = output.matcher();
            if produced_keys.insert(matcher.clone()) {
                let held = self.held_quantity(producer, &matcher);
                let future_without_job = self
                    .future_output_quantity(producer, &matcher)
                    .saturating_sub(
                        job.outputs
                            .iter()
                            .filter(|candidate| candidate.matcher() == matcher)
                            .fold(0u32, |sum, candidate| {
                                sum.saturating_add(candidate.quantity)
                            }),
                    );
                let this_output = job
                    .outputs
                    .iter()
                    .filter(|candidate| candidate.matcher() == matcher)
                    .fold(0u32, |sum, candidate| {
                        sum.saturating_add(candidate.quantity)
                    });
                held.checked_add(future_without_job)
                    .and_then(|sum| sum.checked_add(this_output))
                    .ok_or_else(|| {
                        InventoryError::new(
                            InventoryErrorCode::OutputCapacityReserved,
                            "reserved transform output no longer fits",
                        )
                    })?;
            }
        }

        let consumed = job
            .inputs
            .iter()
            .map(|input| TransformReceiptLine {
                item_id: input.item_id.clone(),
                quantity: input.quantity,
            })
            .collect::<Vec<_>>();
        for input in &job.inputs {
            self.consume_item_quantity_for_job(
                producer,
                &input.item_id,
                input.quantity,
                Some(&job.job_id),
            )?;
        }
        // Release future-output commitments before merging their realization.
        self.transform_jobs.remove(producer);
        let mut produced = Vec::with_capacity(job.outputs.len());
        for (slot, output) in job.outputs.iter().enumerate() {
            let id = self.add_stock(
                producer,
                output,
                &format!("transform:{}:{slot}", job.job_id),
            )?;
            produced.push(TransformReceiptLine {
                item_id: id,
                quantity: output.quantity,
            });
        }
        let receipt = TransformReceipt {
            job_id: job.job_id.clone(),
            producer: producer.clone(),
            consumed,
            produced,
            completed_on_day: current_day,
        };
        self.completed_transform_jobs.insert(
            job.job_id,
            CompletedTransform {
                receipt: receipt.clone(),
                completed_on_day: current_day,
            },
        );
        Ok(receipt)
    }

    pub fn completed_transforms(&self) -> impl Iterator<Item = &CompletedTransform> {
        self.completed_transform_jobs.values()
    }

    pub fn prune_completed_transforms(&mut self, current_day: i64) {
        self.completed_transform_jobs
            .retain(|_, completed| completed.completed_on_day >= current_day.saturating_sub(1));
    }

    /// One atomic, purpose-neutral catalog transaction. The requested lines are
    /// considered in declaration order; matching source stacks are ordered by
    /// `(unit price, item id)`. It may buy a partial target when stock, funds or
    /// the caller's visit budget runs out, but never commits an empty receipt.
    pub fn market_sale(
        &mut self,
        buyer: &ActorId,
        seller: &ActorId,
        requested: &[MarketRequestLine],
        max_spend_sparks: u32,
        operation_key: &str,
    ) -> Result<SaleReceipt, InventoryError> {
        // Staging on a clone makes the composite debit/credit/multi-line move
        // genuinely atomic. Sales are sparse; correctness is worth copying the
        // pure in-memory world, and commit retains exactly one public revision.
        let mut staged = self.clone();
        let receipt =
            staged.market_sale_inner(buyer, seller, requested, max_spend_sparks, operation_key)?;
        let item_id = receipt
            .lines
            .first()
            .map(|line| line.destination_item_id.clone());
        let quantity = receipt
            .lines
            .iter()
            .try_fold(0u32, |sum, line| sum.checked_add(line.quantity))
            .ok_or_else(|| {
                InventoryError::new(
                    InventoryErrorCode::ArithmeticOverflow,
                    "sale quantity overflow",
                )
            })?;
        let position = staged.characters[seller].position_m();
        staged.emit(DomainEvent::world_event(
            "sale",
            seller.clone(),
            Some(buyer.clone()),
            item_id,
            quantity,
            position,
            Vec::new(),
        ));
        staged.touch_public_state();
        *self = staged;
        Ok(receipt)
    }

    fn market_sale_inner(
        &mut self,
        buyer: &ActorId,
        seller: &ActorId,
        requested: &[MarketRequestLine],
        max_spend_sparks: u32,
        operation_key: &str,
    ) -> Result<SaleReceipt, InventoryError> {
        if buyer == seller {
            return Err(InventoryError::new(
                InventoryErrorCode::SelfSale,
                "a vendor cannot buy from itself",
            ));
        }
        if !self.characters.contains_key(buyer) || !self.characters.contains_key(seller) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                "sale actor is missing",
            ));
        }
        if !self.is_present(buyer) || !self.is_present(seller) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                "sale actors must both be present in the city",
            ));
        }
        if max_spend_sparks == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::BudgetExhausted,
                "sale budget is exhausted",
            ));
        }
        let funds = self.spendable_sparks(buyer);
        if funds == 0 {
            return Err(InventoryError::new(
                InventoryErrorCode::InsufficientFunds,
                "buyer has no spendable sparks",
            ));
        }

        #[derive(Clone)]
        struct Planned {
            source: ItemId,
            matcher: ItemMatcher,
            quantity: u32,
            unit_price: u32,
        }
        let mut planned = Vec::<Planned>::new();
        let mut spend = 0u32;
        let cap = funds.min(max_spend_sparks);
        let mut remaining_by_item: BTreeMap<ItemId, u32> = self.characters[seller]
            .holds()
            .iter()
            .map(|id| (id.clone(), self.uncommitted_quantity(id)))
            .collect();

        for request in requested {
            if request.quantity == 0 {
                return Err(InventoryError::new(
                    InventoryErrorCode::BadQuantity,
                    "requested quantity must be positive",
                ));
            }
            let mut candidates: Vec<(u32, ItemId)> = self.characters[seller]
                .holds()
                .iter()
                .filter_map(|id| {
                    let item = self.items.get(id)?;
                    if !request.matcher.matches(item)
                        || remaining_by_item.get(id).copied().unwrap_or(0) == 0
                    {
                        return None;
                    }
                    self.item_catalog
                        .price_sparks(item)
                        .map(|price| (price, id.clone()))
                })
                .collect();
            if candidates.is_empty() {
                // Distinguish exact matching stock whose catalog row lacks a
                // price from an actually empty counter.
                let unpriced = self.characters[seller].holds().iter().any(|id| {
                    self.items.get(id).is_some_and(|item| {
                        request.matcher.matches(item)
                            && self.uncommitted_quantity(id) > 0
                            && self.item_catalog.price_sparks(item).is_none()
                    })
                });
                if unpriced {
                    return Err(InventoryError::new(
                        InventoryErrorCode::UnpricedStock,
                        format!("{} has matching stock with no posted price", seller),
                    ));
                }
                continue;
            }
            candidates.sort();
            let mut wanted = request.quantity;
            for (price, id) in candidates {
                if wanted == 0 || spend >= cap {
                    break;
                }
                if price == 0 {
                    return Err(InventoryError::new(
                        InventoryErrorCode::InvalidContent,
                        "mechanical market stock must have a positive posted price",
                    ));
                }
                let affordable = (cap - spend) / price;
                if affordable == 0 {
                    break;
                }
                let available = remaining_by_item.get(&id).copied().unwrap_or(0);
                let quantity = wanted.min(available).min(affordable);
                if quantity == 0 {
                    continue;
                }
                let line_total = price.checked_mul(quantity).ok_or_else(|| {
                    InventoryError::new(
                        InventoryErrorCode::ArithmeticOverflow,
                        "sale line overflow",
                    )
                })?;
                spend = spend.checked_add(line_total).ok_or_else(|| {
                    InventoryError::new(
                        InventoryErrorCode::ArithmeticOverflow,
                        "sale total overflow",
                    )
                })?;
                *remaining_by_item.get_mut(&id).expect("candidate tracked") -= quantity;
                wanted -= quantity;
                planned.push(Planned {
                    source: id,
                    matcher: request.matcher.clone(),
                    quantity,
                    unit_price: price,
                });
            }
        }
        if planned.is_empty() {
            let any_matching = requested
                .iter()
                .any(|request| self.uncommitted_held_quantity(seller, &request.matcher) > 0);
            let mut cheapest = None;
            for request in requested {
                for id in self.characters[seller].holds() {
                    let Some(item) = self.items.get(id) else {
                        continue;
                    };
                    if request.matcher.matches(item)
                        && self.uncommitted_quantity(id) > 0
                        && let Some(price) = self.item_catalog.price_sparks(item)
                    {
                        cheapest = Some(cheapest.map_or(price, |current: u32| current.min(price)));
                    }
                }
            }
            let (code, message) = if !any_matching {
                (
                    InventoryErrorCode::NoMatchingStock,
                    "the counter has no matching uncommitted stock",
                )
            } else if cheapest.is_some_and(|price| max_spend_sparks < price && funds >= price) {
                (
                    InventoryErrorCode::BudgetExhausted,
                    "no requested unit fits the remaining visit budget",
                )
            } else {
                (
                    InventoryErrorCode::InsufficientFunds,
                    "no requested unit fits the buyer's spendable funds",
                )
            };
            return Err(InventoryError::new(code, message));
        }

        self.debit_sparks(buyer, spend)?;
        self.credit_sparks(seller, spend, &format!("sale:{operation_key}:credit"))?;
        let mut lines = Vec::with_capacity(planned.len());
        for (slot, line) in planned.into_iter().enumerate() {
            let destination = self.transfer_item_quantity(
                seller,
                buyer,
                &line.source,
                line.quantity,
                &format!("sale:{operation_key}:{slot}"),
            )?;
            lines.push(SaleReceiptLine {
                source_item_id: line.source,
                destination_item_id: destination,
                matcher: line.matcher,
                quantity: line.quantity,
                unit_price_sparks: line.unit_price,
                line_total_sparks: line.unit_price * line.quantity,
            });
        }
        Ok(SaleReceipt {
            operation_key: operation_key.to_string(),
            buyer: buyer.clone(),
            seller: seller.clone(),
            lines,
            total_sparks: spend,
        })
    }

    pub fn debit_sparks(&mut self, owner: &ActorId, amount: u32) -> Result<(), InventoryError> {
        if !self.characters.contains_key(owner) {
            return Err(InventoryError::new(
                InventoryErrorCode::UnknownActor,
                format!("unknown wallet owner '{owner}'"),
            ));
        }
        if amount == 0 {
            return Ok(());
        }
        if self.spendable_sparks(owner) < amount {
            return Err(InventoryError::new(
                InventoryErrorCode::InsufficientFunds,
                format!("{owner} lacks {amount} uncommitted sparks"),
            ));
        }
        let ids: Vec<ItemId> = self.characters[owner]
            .holds()
            .iter()
            .filter(|id| {
                self.items
                    .get(*id)
                    .is_some_and(|item| item.kind.as_str() == "spark")
            })
            .cloned()
            .collect();
        let mut remaining = amount;
        for id in ids {
            if remaining == 0 {
                break;
            }
            let take = remaining.min(self.uncommitted_quantity(&id));
            if take > 0 {
                self.consume_item_quantity(owner, &id, take)?;
                remaining -= take;
            }
        }
        debug_assert_eq!(remaining, 0);
        Ok(())
    }

    pub fn credit_sparks(
        &mut self,
        owner: &ActorId,
        amount: u32,
        operation_key: &str,
    ) -> Result<Option<ItemId>, InventoryError> {
        if amount == 0 {
            return Ok(None);
        }
        self.add_stock(
            owner,
            &StockSpec {
                kind: ItemKind::from_raw("spark"),
                metadata: BTreeMap::new(),
                quantity: amount,
            },
            operation_key,
        )
        .map(Some)
    }

    fn deduct_legacy_shares(&mut self, item_id: &ItemId, mut quantity: u32) {
        let Some(shares) = self.legacy_restock_shares.get_mut(item_id) else {
            return;
        };
        shares.sort();
        for share in shares.iter_mut() {
            let take = share.quantity.min(quantity);
            share.quantity -= take;
            quantity -= take;
            if quantity == 0 {
                break;
            }
        }
        shares.retain(|share| share.quantity > 0);
        if shares.is_empty() {
            self.legacy_restock_shares.remove(item_id);
        }
    }

    fn resolve_item_id(&self, parent: &ItemId, operation_key: &str) -> ItemId {
        let mut hasher = DefaultHasher::new();
        "inventory_operation".hash(&mut hasher);
        operation_key.hash(&mut hasher);
        let base = hasher.finish() as i64;
        let mut bump = 0i64;
        loop {
            let candidate = mint_item_id(parent, base.wrapping_add(bump));
            if !self.items.contains_key(&candidate) {
                return candidate;
            }
            bump = bump.wrapping_add(1);
        }
    }
}
