//! Existing ordered dog authority. No seeding, routing, ticking or adoption.
use super::*;
use crate::{
    World,
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod context;
pub(crate) mod records;
pub use context::AnimalsCheckpointContext;
const OWNER: &str = "animals";
pub const MAX_DOGS: usize = 25_000;
pub const MAX_WAYPOINTS: usize = 65_536;
/// Borrowed-ID validation plus fixed serializer/error working state.
pub const VALIDATION_WORKING_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AnimalsCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for AnimalsCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
pub(crate) fn id(s: &str) -> Result<()> {
    check(
        s.len() <= 4 * crate::MAX_ID_CHARS && crate::ids::is_valid_id(s),
        "invalid animal identity",
    )
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<AnimalsCost> {
    let cost = AnimalsCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn context_for_export(
    c: AnimalsCheckpointContext<'_>,
    r: &mut Reservation,
) -> Result<context::BindingV1> {
    r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    if r.bytes() < aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES {
        r.resize(aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES)?;
    }
    context::BindingV1::new(c)
}
fn validate(dogs: &[Dog]) -> Result<()> {
    check(dogs.len() <= MAX_DOGS, "dog count limit")?;
    // Count and per-record bounds precede the only allocated validation index.
    for dog in dogs {
        id(dog.id.as_str())?;
        check(
            dog.name.len() <= checkpoint::records::MAX_TEXT_BYTES
                && dog.description.len() <= checkpoint::records::MAX_TEXT_BYTES,
            "dog prose byte limit",
        )?;
        check(dog.path.len() <= MAX_WAYPOINTS, "dog path count limit")?;
        crate::character::checkpoint::point(dog.base)?;
        crate::character::checkpoint::point(dog.position_m)?;
        for p in &dog.path {
            crate::character::checkpoint::point(*p)?;
        }
        check(
            dog.build.is_finite()
                && [
                    dog.leash_m,
                    dog.facing_yaw,
                    dog.speed,
                    dog.gait_phase,
                    dog.rest_s,
                ]
                .iter()
                .all(|v| v.is_finite()),
            "nonfinite dog numeric state",
        )?;
        // Public edits and historical paths survive verbatim, including signed
        // values and non-walk-plane coordinates. Arrival can have positive speed
        // with an empty path. Epoch intentionally wraps in ordinary step_dogs.
    }
    let mut ids: Vec<_> = dogs.iter().map(|dog| &dog.id).collect();
    ids.sort_unstable();
    check(
        ids.windows(2).all(|pair| pair[0] != pair[1]),
        "duplicate dog identity",
    )?;
    Ok(())
}
#[derive(Debug, Serialize)]
pub struct WorldAnimalsDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "records::dog::vec")]
    pub(crate) dogs: Vec<Dog>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "records::dog::vec")]
    dogs: Vec<Dog>,
}
impl From<WorldWire> for WorldAnimalsDtoV1 {
    fn from(w: WorldWire) -> Self {
        Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            dogs: w.dogs,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct WorldView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "records::dog::vec")]
    dogs: &'a [Dog],
}
impl<'a> WorldView<'a> {
    pub(crate) fn new(w: &'a World, now: LogicalTime, context: &'a context::BindingV1) -> Self {
        Self {
            version: 1,
            boundary: now,
            context,
            dogs: &w.dogs,
        }
    }
}
#[derive(Debug)]
pub struct WorldAnimalsCandidate {
    pub(crate) data: WorldAnimalsDtoV1,
}
impl World {
    pub fn export_animals_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldAnimalsDtoV1>> {
        let c = AnimalsCheckpointContext::from_world(self, now);
        let context = context_for_export(c, &mut r)?;
        prepare(&WorldView::new(self, now, &context), &mut r)?;
        validate(&self.dogs)?;
        Ok(Admitted::new(
            WorldAnimalsDtoV1 {
                version: 1,
                boundary: now,
                context,
                dogs: self.dogs.clone(),
            },
            r,
        ))
    }
}
impl WorldAnimalsDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: AnimalsCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self::from(w);
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: AnimalsCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported animals version")?;
        check(self.boundary == c.now, "animals boundary disagreement")?;
        check(
            self.context == context::BindingV1::new(c)?,
            "animals nav context disagreement",
        )?;
        validate(&self.dogs)
    }
    pub fn cost(&self) -> Result<AnimalsCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: AnimalsCheckpointContext<'_>) -> AnimalsCounts {
        AnimalsCounts {
            characters: c.backbone.characters.len(),
            dogs: self.dogs.len(),
            path_dogs: self.dogs.iter().filter(|d| !d.path.is_empty()).count(),
            waypoints: self.dogs.iter().map(|d| d.path.len()).sum(),
            resting: self
                .dogs
                .iter()
                .filter(|d| d.path.is_empty() && d.speed == 0.0)
                .count(),
            moving: self.dogs.iter().filter(|d| d.speed != 0.0).count(),
            turning: self
                .dogs
                .iter()
                .filter(|d| !d.path.is_empty() && d.speed == 0.0)
                .count(),
            stop_pending: self
                .dogs
                .iter()
                .filter(|d| d.path.is_empty() && d.speed != 0.0)
                .count(),
            decided: self.dogs.iter().filter(|d| d.epoch != 0).count(),
        }
    }
}
impl Admitted<WorldAnimalsDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: AnimalsCheckpointContext<'_>,
    ) -> Result<Admitted<WorldAnimalsCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(WorldAnimalsCandidate { data: d })
        })
    }
}
impl WorldAnimalsCandidate {
    pub fn dogs(&self) -> &[Dog] {
        &self.data.dogs
    }
    pub fn counts(&self, c: AnimalsCheckpointContext<'_>) -> AnimalsCounts {
        self.data.counts(c)
    }
}
pub(crate) struct WorldAnimalsV1;
impl WorldAnimalsV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &WorldAnimalsDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WorldAnimalsDtoV1, D::Error> {
        WorldWire::deserialize(d).map(Into::into)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AnimalsCounts {
    pub characters: usize,
    pub dogs: usize,
    pub path_dogs: usize,
    pub waypoints: usize,
    pub resting: usize,
    pub moving: usize,
    pub turning: usize,
    pub stop_pending: usize,
    pub decided: usize,
}
#[cfg(test)]
pub(crate) mod tests;
