//! Dog authority, exact publication flag and the shared human/dog elapsed anchor.
//! No complete Engine hydration or public partial installation is exposed.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate, serde_support::required_option},
    dogs::checkpoint::{self as owner, AnimalsCheckpointContext, AnimalsCost, WorldAnimalsDtoV1},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
const OWNER: &str = "engine_animals";
#[derive(Debug, Serialize)]
pub struct EngineAnimalsDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldAnimalsV1")]
    world: WorldAnimalsDtoV1,
    player_id: ActorId,
    config_nav: Option<[u8; 32]>,
    movement_now: LogicalTime,
    dogs_published: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldAnimalsV1")]
    world: WorldAnimalsDtoV1,
    player_id: ActorId,
    #[serde(deserialize_with = "required_option")]
    config_nav: Option<[u8; 32]>,
    movement_now: LogicalTime,
    dogs_published: bool,
}
#[derive(Serialize)]
struct View<'a, W: Serialize> {
    version: u16,
    boundary: LogicalTime,
    world: W,
    player_id: &'a ActorId,
    config_nav: Option<[u8; 32]>,
    movement_now: LogicalTime,
    dogs_published: bool,
}
fn elapsed(n: f64) -> Result<LogicalTime> {
    crate::checkpoint::logical(OWNER, n)?;
    Ok(LogicalTime::new(n).expect("validated elapsed anchor"))
}
impl<'a, W: Serialize> View<'a, W> {
    fn new(e: &'a Engine, now: LogicalTime, world: W) -> Result<Self> {
        Ok(Self {
            version: 1,
            boundary: now,
            world,
            player_id: &e.config.player_id,
            config_nav: e.config.nav.as_deref().map(NavData::checkpoint_fingerprint),
            movement_now: elapsed(e.movement_now)?,
            dogs_published: e.dogs_published,
        })
    }
}
#[derive(Debug)]
pub struct EngineAnimalsCandidate {
    data: EngineAnimalsDtoV1,
}
impl Engine {
    pub fn checkpoint_animals_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<AnimalsCost>> {
        let c = AnimalsCheckpointContext::from_world(&self.world, now)
            .with_engine_nav(self.config.nav.as_deref());
        let binding = owner::context_for_export(c, &mut r)?;
        let cost = owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding))?,
            &mut r,
        )?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_animals_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineAnimalsDtoV1>> {
        let c = AnimalsCheckpointContext::from_world(&self.world, now)
            .with_engine_nav(self.config.nav.as_deref());
        let binding = owner::context_for_export(c, &mut r)?;
        owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding))?,
            &mut r,
        )?;
        self.world
            .export_animals_checkpoint(now, r)?
            .try_map(|world, _| {
                let d = EngineAnimalsDtoV1 {
                    version: 1,
                    boundary: now,
                    world,
                    player_id: self.config.player_id.clone(),
                    config_nav: self
                        .config
                        .nav
                        .as_deref()
                        .map(NavData::checkpoint_fingerprint),
                    movement_now: elapsed(self.movement_now)?,
                    dogs_published: self.dogs_published,
                };
                d.validate(c)?;
                Ok(d)
            })
    }
}
impl EngineAnimalsDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: AnimalsCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, owner::VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            player_id: w.player_id,
            config_nav: w.config_nav,
            movement_now: w.movement_now,
            dogs_published: w.dogs_published,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: AnimalsCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine animals version")?;
        owner::check(
            self.boundary == c.now && self.world.boundary == c.now,
            "Engine animals boundary disagreement",
        )?;
        self.world.validate(c)?;
        owner::id(self.player_id.as_str())?;
        owner::check(
            c.backbone.characters.contains_key(&self.player_id),
            "missing animals player binding",
        )?;
        owner::check(
            self.config_nav == c.engine_nav.map(NavData::checkpoint_fingerprint),
            "Engine animals nav binding disagreement",
        )?;
        elapsed(self.movement_now.seconds())?;
        // The shared anchor is retained exactly. Nav=None leaves it untouched,
        // backwards debug polls may put it after boundary, and empty packs can
        // retain either publication flag after public edits. Do not normalize.
        Ok(())
    }
    pub fn cost(&self) -> Result<AnimalsCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: AnimalsCheckpointContext<'_>) -> EngineAnimalsCounts {
        EngineAnimalsCounts {
            animals: self.world.counts(c),
            dogs_published: self.dogs_published,
            engine_nav: self.config_nav.is_some(),
        }
    }
}
impl Admitted<EngineAnimalsDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: AnimalsCheckpointContext<'_>,
    ) -> Result<Admitted<EngineAnimalsCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineAnimalsCandidate { data: d })
        })
    }
}
impl EngineAnimalsCandidate {
    pub fn dogs(&self) -> &[crate::dogs::Dog] {
        &self.data.world.dogs
    }
    pub fn dogs_published(&self) -> bool {
        self.data.dogs_published
    }
    pub fn movement_now(&self) -> LogicalTime {
        self.data.movement_now
    }
    pub fn counts(&self, c: AnimalsCheckpointContext<'_>) -> EngineAnimalsCounts {
        self.data.counts(c)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineAnimalsCounts {
    #[serde(flatten)]
    pub animals: owner::AnimalsCounts,
    pub dogs_published: bool,
    pub engine_nav: bool,
}
#[cfg(test)]
mod tests;
