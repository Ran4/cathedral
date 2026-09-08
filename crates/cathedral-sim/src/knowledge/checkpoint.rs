//! Private knowledge components. No seeding, projection refresh, stale-fact
//! sweep or production World adoption occurs while validating a candidate.
use super::*;
use crate::{
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    clock::WorldTime,
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub mod context;
pub use context::KnowledgeCheckpointContext;
pub(crate) mod records;
pub(crate) mod validate;
const OWNER: &str = "knowledge";
pub const MAX_NAMES: usize = 25_000;
/// Fixed installed-home parsing/mark sorting allowance for uncached centroids.
pub const DEFINITION_WORKING_BYTES: usize = 4 * 1024 * 1024;
/// Combined validation allowance. The geometry pass fits 256 KiB; adjacency
/// may retain full nearest-query capacity per row and uses the combined bound.
/// Adjacency and home parsing have sequential lifetimes. The full charge stays
/// attached even for empty DTOs and after candidate conversion.
pub const VALIDATION_WORKING_BYTES: usize = 256 * 1024 + DEFINITION_WORKING_BYTES;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KnowledgeCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for KnowledgeCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn error(reason: &str) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(reason)) }
}
pub(crate) fn id(s: &str) -> Result<()> {
    check(crate::ids::is_valid_id(s), "invalid knowledge identity")
}
pub(crate) fn text(s: &str) -> Result<()> {
    check(
        s.len() <= crate::checkpoint::records::MAX_TEXT_BYTES,
        "knowledge text byte limit",
    )
}
pub(crate) fn prepare<T: Serialize>(value: &T, r: &mut Reservation) -> Result<KnowledgeCost> {
    let cost = KnowledgeCost::from(aggregate::prepare_export(value, OWNER, r)?);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn context_for_export(
    c: KnowledgeCheckpointContext<'_>,
    r: &mut Reservation,
) -> Result<context::BindingV1> {
    r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    if r.bytes() < aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES {
        r.resize(aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES)?;
    }
    context::BindingV1::new(c)
}
fn cloned(k: &Knowledge) -> Knowledge {
    let mut k = k.clone();
    k.holdings = Arc::new((*k.holdings).clone());
    k.air = Arc::new((*k.air).clone());
    k
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KnowledgeCounts {
    pub characters: usize,
    pub facts: usize,
    pub holding_actors: usize,
    pub holdings: usize,
    pub air: usize,
    pub occasions: usize,
    pub player_receipts: usize,
    pub seated_keys: usize,
    pub stage_tellings: usize,
    pub hearsay_raised: usize,
    pub adjacency_rows: usize,
}
pub(crate) fn counts(
    k: &Knowledge,
    c: KnowledgeCheckpointContext<'_>,
    adjacency_rows: usize,
) -> KnowledgeCounts {
    KnowledgeCounts {
        characters: c.backbone.characters.len(),
        facts: k.live.len(),
        holding_actors: k.holdings.len(),
        holdings: k.holdings.values().map(Vec::len).sum(),
        air: k.air.len(),
        occasions: k.occasions.len(),
        player_receipts: k.player_learned.len(),
        seated_keys: k.seated.as_ref().map_or(0, |(_, k)| k.len()),
        stage_tellings: k.player_stage_tellings.len(),
        hearsay_raised: k.hearsay_raised.len(),
        adjacency_rows,
    }
}
#[derive(Debug, Serialize)]
pub struct KnowledgeDtoV1 {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::KnowledgeV1")]
    knowledge: Knowledge,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KnowledgeWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::KnowledgeV1")]
    knowledge: Knowledge,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::KnowledgeV1")]
    knowledge: &'a Knowledge,
}
#[derive(Debug)]
pub struct KnowledgeCandidate {
    pub(crate) data: KnowledgeDtoV1,
}
impl Knowledge {
    pub fn export_checkpoint(
        &self,
        c: KnowledgeCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<KnowledgeDtoV1>> {
        let context = context_for_export(c, &mut r)?;
        prepare(
            &View {
                version: 1,
                boundary: c.now,
                context: &context,
                sampled_time: c.backbone.current_time,
                knowledge: self,
            },
            &mut r,
        )?;
        validate::boundary(1, c.now, &context, c.backbone.current_time, c)?;
        validate::knowledge(self, c)?;
        Ok(Admitted::new(
            KnowledgeDtoV1 {
                version: 1,
                boundary: c.now,
                context,
                sampled_time: c.backbone.current_time,
                knowledge: cloned(self),
            },
            r,
        ))
    }
}
impl KnowledgeDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: KnowledgeWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            knowledge: w.knowledge,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: KnowledgeCheckpointContext<'_>) -> Result<()> {
        validate::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate::knowledge(&self.knowledge, c)
    }
    pub fn cost(&self) -> Result<KnowledgeCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: KnowledgeCheckpointContext<'_>) -> KnowledgeCounts {
        counts(&self.knowledge, c, 0)
    }
}
impl Admitted<KnowledgeDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<KnowledgeCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(KnowledgeCandidate { data: d })
        })
    }
}
impl KnowledgeCandidate {
    pub fn knowledge(&self) -> &Knowledge {
        &self.data.knowledge
    }
}
#[derive(Debug, Serialize)]
pub struct WorldKnowledgeDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    pub(crate) knowledge_enabled: bool,
    pub(crate) pollen_no_salience: bool,
    #[serde(with = "records::KnowledgeV1")]
    pub(crate) knowledge: Knowledge,
    #[serde(with = "super::pollen::checkpoint::AdjacencyV1")]
    pub(crate) area_adjacency: AreaAdjacency,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    knowledge_enabled: bool,
    pollen_no_salience: bool,
    #[serde(with = "records::KnowledgeV1")]
    knowledge: Knowledge,
    #[serde(with = "super::pollen::checkpoint::AdjacencyV1")]
    area_adjacency: AreaAdjacency,
}
#[derive(Serialize)]
pub(crate) struct WorldView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    knowledge_enabled: bool,
    pollen_no_salience: bool,
    #[serde(with = "records::KnowledgeV1")]
    knowledge: &'a Knowledge,
    #[serde(with = "super::pollen::checkpoint::AdjacencyV1")]
    area_adjacency: &'a AreaAdjacency,
}
impl<'a> WorldView<'a> {
    pub(crate) fn new(w: &'a World, now: LogicalTime, context: &'a context::BindingV1) -> Self {
        Self {
            version: 1,
            boundary: now,
            context,
            sampled_time: w.current_time,
            knowledge_enabled: w.knowledge_enabled,
            pollen_no_salience: w.pollen_no_salience,
            knowledge: &w.knowledge,
            area_adjacency: &w.area_adjacency,
        }
    }
}
pub(crate) struct WorldKnowledgeV1;
impl WorldKnowledgeV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &WorldKnowledgeDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WorldKnowledgeDtoV1, D::Error> {
        let w = WorldWire::deserialize(d)?;
        Ok(WorldKnowledgeDtoV1 {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            knowledge_enabled: w.knowledge_enabled,
            pollen_no_salience: w.pollen_no_salience,
            knowledge: w.knowledge,
            area_adjacency: w.area_adjacency,
        })
    }
}
#[derive(Debug)]
pub struct WorldKnowledgeCandidate {
    pub(crate) data: WorldKnowledgeDtoV1,
}
impl World {
    pub fn checkpoint_knowledge_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<KnowledgeCost> {
        let c = KnowledgeCheckpointContext::from_world(self, now);
        let context = context_for_export(c, &mut r)?;
        prepare(
            &WorldView {
                version: 1,
                boundary: now,
                context: &context,
                sampled_time: self.current_time,
                knowledge_enabled: self.knowledge_enabled,
                pollen_no_salience: self.pollen_no_salience,
                knowledge: &self.knowledge,
                area_adjacency: &self.area_adjacency,
            },
            &mut r,
        )
    }
    pub fn export_knowledge_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldKnowledgeDtoV1>> {
        let c = KnowledgeCheckpointContext::from_world(self, now);
        let context = context_for_export(c, &mut r)?;
        prepare(
            &WorldView {
                version: 1,
                boundary: now,
                context: &context,
                sampled_time: self.current_time,
                knowledge_enabled: self.knowledge_enabled,
                pollen_no_salience: self.pollen_no_salience,
                knowledge: &self.knowledge,
                area_adjacency: &self.area_adjacency,
            },
            &mut r,
        )?;
        validate::boundary(1, now, &context, self.current_time, c)?;
        validate::knowledge(&self.knowledge, c)?;
        super::pollen::checkpoint::validate(&self.area_adjacency, c.areas, false)?;
        Ok(Admitted::new(
            WorldKnowledgeDtoV1 {
                version: 1,
                boundary: now,
                context,
                sampled_time: self.current_time,
                knowledge_enabled: self.knowledge_enabled,
                pollen_no_salience: self.pollen_no_salience,
                knowledge: cloned(&self.knowledge),
                area_adjacency: (*self.area_adjacency).clone(),
            },
            r,
        ))
    }
}
impl WorldKnowledgeDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            knowledge_enabled: w.knowledge_enabled,
            pollen_no_salience: w.pollen_no_salience,
            knowledge: w.knowledge,
            area_adjacency: w.area_adjacency,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: KnowledgeCheckpointContext<'_>) -> Result<()> {
        validate::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate::knowledge(&self.knowledge, c)?;
        super::pollen::checkpoint::validate(&self.area_adjacency, c.areas, false)
    }
    pub fn cost(&self) -> Result<KnowledgeCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: KnowledgeCheckpointContext<'_>) -> KnowledgeCounts {
        counts(
            &self.knowledge,
            c,
            super::pollen::checkpoint::rows(&self.area_adjacency),
        )
    }
}
impl Admitted<WorldKnowledgeDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<WorldKnowledgeCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(WorldKnowledgeCandidate { data: d })
        })
    }
}
impl WorldKnowledgeCandidate {
    pub fn knowledge(&self) -> &Knowledge {
        &self.data.knowledge
    }
    pub fn counts(&self, c: KnowledgeCheckpointContext<'_>) -> KnowledgeCounts {
        self.data.counts(c)
    }
}

pub(crate) fn history_counts(k: &Knowledge) -> (usize, usize, usize) {
    (
        k.player_learned
            .keys()
            .filter(|id| !k.by_id.contains_key(*id))
            .count(),
        k.seated.as_ref().map_or(0, |(_, keys)| {
            keys.iter().filter(|key| !k.live.contains_key(key)).count()
        }),
        k.occasions.values().filter(|o| o.offered).count(),
    )
}

#[cfg(test)]
pub(crate) mod tests;
