//! Private live item/quantity authority. Round recipe/planner cross-validation
//! remains a required later composition gate; this owner never recreates jobs.
use super::*;
use crate::{
    Character, Presence,
    checkpoint::{
        Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate, records,
        serde_support::remote_adapters,
    },
    item::ItemCatalog,
    offer::Offer,
};
const OWNER: &str = "inventory";
struct SharesV1;
impl SharesV1 {
    fn serialize<S: serde::Serializer>(
        v: &Vec<LegacyRestockShare>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        records::share::vec::serialize(v, s)
    }
    fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<LegacyRestockShare>, D::Error> {
        records::share::vec::deserialize(d)
    }
}
remote_adapters!(shares, Vec<LegacyRestockShare>, SharesV1);
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryDtoV1 {
    version: u16,
    catalog: [u8; 32],
    #[serde(with = "records::item::map")]
    pub(crate) items: BTreeMap<ItemId, Item>,
    #[serde(with = "records::offer::map")]
    pub(crate) offers: BTreeMap<ItemId, Offer>,
    #[serde(with = "shares::map")]
    pub(crate) legacy_restock_shares: BTreeMap<ItemId, Vec<LegacyRestockShare>>,
    #[serde(with = "records::job::map")]
    pub(crate) transform_jobs: BTreeMap<ActorId, TransformJob>,
    #[serde(with = "records::completed::map")]
    pub(crate) completed_transform_jobs: BTreeMap<String, CompletedTransform>,
}
// Deserialization is private so external callers cannot bypass aggregate
// admission with serde_json::from_slice::<PublicDto>().
#[derive(Serialize, Deserialize)]
#[serde(remote = "InventoryDtoV1", deny_unknown_fields)]
pub(crate) struct InventoryWireV1 {
    version: u16,
    catalog: [u8; 32],
    #[serde(with = "records::item::map")]
    pub(crate) items: BTreeMap<ItemId, Item>,
    #[serde(with = "records::offer::map")]
    pub(crate) offers: BTreeMap<ItemId, Offer>,
    #[serde(with = "shares::map")]
    pub(crate) legacy_restock_shares: BTreeMap<ItemId, Vec<LegacyRestockShare>>,
    #[serde(with = "records::job::map")]
    pub(crate) transform_jobs: BTreeMap<ActorId, TransformJob>,
    #[serde(with = "records::completed::map")]
    pub(crate) completed_transform_jobs: BTreeMap<String, CompletedTransform>,
}
#[derive(Deserialize)]
struct Decoded(#[serde(with = "InventoryWireV1")] InventoryDtoV1);

#[derive(Serialize)]
pub(crate) struct View<'a> {
    version: u16,
    catalog: [u8; 32],
    #[serde(with = "records::item::map")]
    items: &'a BTreeMap<ItemId, Item>,
    #[serde(with = "records::offer::map")]
    offers: &'a BTreeMap<ItemId, Offer>,
    #[serde(with = "shares::map")]
    legacy_restock_shares: &'a BTreeMap<ItemId, Vec<LegacyRestockShare>>,
    #[serde(with = "records::job::map")]
    transform_jobs: &'a BTreeMap<ActorId, TransformJob>,
    #[serde(with = "records::completed::map")]
    completed_transform_jobs: &'a BTreeMap<String, CompletedTransform>,
}
impl<'a> View<'a> {
    pub(crate) fn new(w: &'a World) -> Self {
        Self {
            version: 1,
            catalog: w.item_catalog.checkpoint_fingerprint(),
            items: &w.items,
            offers: &w.offers,
            legacy_restock_shares: &w.legacy_restock_shares,
            transform_jobs: &w.transform_jobs,
            completed_transform_jobs: &w.completed_transform_jobs,
        }
    }
}
impl World {
    pub fn export_inventory_checkpoint(
        &self,
        mut r: Reservation,
    ) -> Result<Admitted<InventoryDtoV1>> {
        aggregate::prepare_export(&View::new(self), OWNER, &mut r)?;
        let dto = InventoryDtoV1::from_world(self);
        dto.validate(&self.characters, &self.item_catalog, self.event_sequence)?;
        Ok(Admitted::new(dto, r))
    }
}
impl InventoryDtoV1 {
    pub(crate) fn from_world(w: &World) -> Self {
        Self {
            version: 1,
            catalog: w.item_catalog.checkpoint_fingerprint(),
            items: w.items.clone(),
            offers: w.offers.clone(),
            legacy_restock_shares: w.legacy_restock_shares.clone(),
            transform_jobs: w.transform_jobs.clone(),
            completed_transform_jobs: w.completed_transform_jobs.clone(),
        }
    }
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        characters: &BTreeMap<ActorId, Character>,
        catalog: &ItemCatalog,
        event_sequence: i64,
    ) -> Result<Admitted<Self>> {
        let Decoded(dto) = aggregate::decode(bytes, OWNER, &mut r)?;
        dto.validate(characters, catalog, event_sequence)?;
        Ok(Admitted::new(dto, r))
    }
    pub fn cost(&self) -> Result<ComponentCost> {
        aggregate::measure(self, OWNER)
    }
    pub fn validate(
        &self,
        characters: &BTreeMap<ActorId, Character>,
        catalog: &ItemCatalog,
        event_sequence: i64,
    ) -> Result<()> {
        check(self.version == 1, "unsupported inventory version")?;
        check(
            self.catalog == catalog.checkpoint_fingerprint(),
            "item catalog incompatibility",
        )?;
        check(self.items.len() <= 250_000, "item count exceeds v1 policy")?;
        let mut owners = BTreeMap::new();
        let mut committed = BTreeMap::<&ItemId, u32>::new();
        for (actor_id, actor) in characters {
            let mut stuff = BTreeSet::new();
            let mut pockets = BTreeMap::new();
            for id in actor.holds() {
                let item = self
                    .items
                    .get(id)
                    .ok_or_else(|| err("held item is missing"))?;
                check(
                    owners.insert(id, actor_id).is_none(),
                    "item has duplicate/multiple holders",
                )?;
                if catalog.stackable(item) {
                    check(
                        stuff.insert((&item.kind, &item.metadata)),
                        "same-stuff stacks were not merged",
                    )?;
                }
            }
            for p in actor.pockets() {
                check(
                    owners.get(&p.item_id) == Some(&actor_id),
                    "pocket item is not held by actor",
                )?;
                check(actor.has_body_slot(p.slot), "pocket slot unavailable")?;
                check(
                    catalog.size(&self.items[&p.item_id]) == crate::item::ItemSize::Palmable,
                    "pocket item is not palmable",
                )?;
                let count = pockets.entry(p.slot).or_insert(0);
                *count += 1;
                check(
                    *count <= crate::POCKET_SLOT_CAPACITY,
                    "pocket capacity exceeded",
                )?;
                add(&mut committed, &p.item_id, 1)?;
            }
            for g in &actor.state.gut {
                // The current gut owner retains kind+metadata, not an exact swallowed ID.
                let probe = Item {
                    id: ItemId::from_raw("checkpoint_gut"),
                    kind: g.kind.clone(),
                    quantity: 1,
                    metadata: g.metadata.clone(),
                };
                catalog
                    .validate_seed_item(&probe)
                    .map_err(|_| err("invalid gut catalog binding"))?;
            }
        }
        for (id, item) in &self.items {
            check(
                id == &item.id && id.is_valid() && item.kind.is_valid() && item.quantity > 0,
                "invalid item identity/quantity",
            )?;
            crate::character::checkpoint::metadata(&item.metadata)?;
            catalog
                .validate_seed_item(item)
                .map_err(|_| err("invalid item catalog binding"))?;
            check(owners.contains_key(id), "live item has no holder")?;
        }
        for (id, o) in &self.offers {
            check(
                id == &o.item_id && o.quantity > 0,
                "invalid offer identity/quantity",
            )?;
            check(
                owners.get(id) == Some(&&o.giver_id),
                "offer giver is not item owner",
            )?;
            let present = |id: &ActorId| {
                characters
                    .get(id)
                    .is_some_and(|c| c.state.presence == Presence::InCity)
            };
            check(
                present(&o.giver_id)
                    && o.target_id.as_ref() != Some(&o.giver_id)
                    && o.target_id.as_ref().is_none_or(present),
                "offer actor presence/target mismatch",
            )?;
            check(
                o.created_seq > 0 && o.created_seq <= event_sequence,
                "offer sequence outside saved event history",
            )?;
            add(&mut committed, id, o.quantity)?;
        }
        for (id, shares) in &self.legacy_restock_shares {
            let item = self
                .items
                .get(id)
                .ok_or_else(|| err("restock share references missing item"))?;
            let mut sum = 0u32;
            let mut previous = None;
            for share in shares {
                check(
                    share.quantity > 0 && owners.get(id) == Some(&&share.original_vendor),
                    "invalid restock ownership/quantity",
                )?;
                name(&share.source_id)?;
                let key = (&share.source_id, &share.original_vendor);
                check(
                    previous.is_none_or(|p| p < key),
                    "restock shares not unique and sorted",
                )?;
                previous = Some(key);
                sum = sum
                    .checked_add(share.quantity)
                    .ok_or_else(|| err("restock share overflow"))?;
            }
            check(sum <= item.quantity, "restock shares exceed item quantity")?;
        }
        let mut job_ids = BTreeSet::new();
        for (producer, j) in &self.transform_jobs {
            name(&j.job_id)?;
            name(&j.spec_id)?;
            check(
                job_ids.insert(&j.job_id) && !self.completed_transform_jobs.contains_key(&j.job_id),
                "duplicate active/completed transform identity",
            )?;
            check(
                producer == &j.producer && characters.contains_key(producer),
                "invalid transform producer",
            )?;
            check(
                !j.inputs.is_empty() && !j.outputs.is_empty(),
                "transform has empty plan",
            )?;
            check(
                j.progress_work_minutes.is_finite()
                    && (0.0..=1.0e12).contains(&j.progress_work_minutes),
                "invalid transform progress",
            )?;
            day(j.production_day)?;
            // Repeated input IDs are legal ordered lines; aggregate their commitments.
            for input in &j.inputs {
                check(
                    input.quantity > 0 && owners.get(&input.item_id) == Some(&producer),
                    "invalid reserved input ownership/quantity",
                )?;
                add(&mut committed, &input.item_id, input.quantity)?;
            }
            let mut outputs =
                BTreeMap::<(&crate::item::ItemKind, &BTreeMap<String, String>), u32>::new();
            for output in &j.outputs {
                let probe = Item {
                    id: ItemId::from_raw("checkpoint_output"),
                    kind: output.kind.clone(),
                    quantity: output.quantity,
                    metadata: output.metadata.clone(),
                };
                crate::character::checkpoint::metadata(&output.metadata)?;
                catalog
                    .validate_seed_item(&probe)
                    .map_err(|_| err("invalid transform output"))?;
                let sum = outputs.entry((&output.kind, &output.metadata)).or_default();
                *sum = sum
                    .checked_add(output.quantity)
                    .ok_or_else(|| err("planned output overflow"))?;
            }
            for ((kind, metadata), future) in outputs {
                let mut total = future;
                for id in characters[producer].holds() {
                    let item = &self.items[id];
                    if &item.kind == kind && &item.metadata == metadata {
                        total = total
                            .checked_add(item.quantity)
                            .ok_or_else(|| err("held plus planned output overflow"))?;
                    }
                }
            }
        }
        for (id, quantity) in committed {
            check(
                quantity <= self.items[id].quantity,
                "pocket/offer/transform commitments exceed stack",
            )?;
        }
        for (id, c) in &self.completed_transform_jobs {
            name(id)?;
            check(
                id == &c.receipt.job_id && c.completed_on_day == c.receipt.completed_on_day,
                "completed transform identity/day mismatch",
            )?;
            day(c.completed_on_day)?;
            check(
                c.receipt.producer.is_valid(),
                "invalid historical producer identity",
            )?;
            // Completed lineage remains historical after consumption, merge or transfer.
            for lines in [&c.receipt.consumed, &c.receipt.produced] {
                check(!lines.is_empty(), "empty completed lineage")?;
                let mut totals = BTreeMap::new();
                for line in lines {
                    check(
                        line.item_id.is_valid() && line.quantity > 0,
                        "invalid historical lineage",
                    )?;
                    add(&mut totals, &line.item_id, line.quantity)?;
                }
            }
        }
        Ok(())
    }
}
impl Admitted<InventoryDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|dto, r| aggregate::encode(&dto, OWNER, r))
    }
}
fn err(reason: &'static str) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn check(ok: bool, reason: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(err(reason)) }
}
fn name(v: &str) -> Result<()> {
    check(
        !v.is_empty() && v.len() <= records::MAX_TEXT_BYTES && !v.chars().any(char::is_control),
        "invalid inventory plan identity",
    )
}
fn day(v: i64) -> Result<()> {
    check(
        (-1_000_000..=1_000_000).contains(&v),
        "inventory calendar day outside v1 range",
    )
}
fn add<'a>(values: &mut BTreeMap<&'a ItemId, u32>, id: &'a ItemId, n: u32) -> Result<()> {
    let v = values.entry(id).or_default();
    *v = v
        .checked_add(n)
        .ok_or_else(|| err("inventory commitment overflow"))?;
    Ok(())
}
