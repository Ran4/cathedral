//! Existing marks authority and exact publication cache, with no partial adoption.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate},
    marks::checkpoint::{self as owner, MarksCheckpointContext, MarksCost, WorldMarksDtoV1},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod records;
const OWNER: &str = "engine_marks";
#[derive(Debug, Serialize)]
pub struct EngineMarksDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldMarksV1")]
    world: WorldMarksDtoV1,
    player_id: ActorId,
    config_marks_enabled: bool,
    #[serde(with = "owner::records::SwitchesV1")]
    config_mark_kinds: crate::marks::MarkKindSwitches,
    #[serde(with = "owner::records::float_bits")]
    config_marks_decay_scale: f64,
    #[serde(with = "records::standing::option")]
    last_chalk_standing: Option<EngineMessage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldMarksV1")]
    world: WorldMarksDtoV1,
    player_id: ActorId,
    config_marks_enabled: bool,
    #[serde(with = "owner::records::SwitchesV1")]
    config_mark_kinds: crate::marks::MarkKindSwitches,
    #[serde(with = "owner::records::float_bits")]
    config_marks_decay_scale: f64,
    #[serde(with = "records::standing::option")]
    last_chalk_standing: Option<EngineMessage>,
}
#[derive(Serialize)]
struct View<'a, W: Serialize> {
    version: u16,
    boundary: LogicalTime,
    world: W,
    player_id: &'a ActorId,
    config_marks_enabled: bool,
    #[serde(with = "owner::records::SwitchesV1")]
    config_mark_kinds: crate::marks::MarkKindSwitches,
    #[serde(with = "owner::records::float_bits")]
    config_marks_decay_scale: f64,
    #[serde(with = "records::standing::option")]
    last_chalk_standing: &'a Option<EngineMessage>,
}
impl<'a, W: Serialize> View<'a, W> {
    fn new(e: &'a Engine, now: LogicalTime, world: W) -> Self {
        Self {
            version: 1,
            boundary: now,
            world,
            player_id: &e.config.player_id,
            config_marks_enabled: e.config.marks_enabled,
            config_mark_kinds: e.config.mark_kinds,
            config_marks_decay_scale: e.config.marks_decay_scale,
            last_chalk_standing: &e.last_chalk_standing,
        }
    }
}
#[derive(Debug)]
pub struct EngineMarksCandidate {
    data: EngineMarksDtoV1,
}
impl Engine {
    pub fn checkpoint_marks_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<MarksCost>> {
        let c = MarksCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        let cost = owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding)),
            &mut r,
        )?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_marks_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineMarksDtoV1>> {
        let c = MarksCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding)),
            &mut r,
        )?;
        self.world
            .export_marks_checkpoint(now, r)?
            .try_map(|world, _| {
                let d = EngineMarksDtoV1 {
                    version: 1,
                    boundary: now,
                    world,
                    player_id: self.config.player_id.clone(),
                    config_marks_enabled: self.config.marks_enabled,
                    config_mark_kinds: self.config.mark_kinds,
                    config_marks_decay_scale: self.config.marks_decay_scale,
                    last_chalk_standing: self.last_chalk_standing.clone(),
                };
                d.validate(c)?;
                Ok(d)
            })
    }
}
impl EngineMarksDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: MarksCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, owner::VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            player_id: w.player_id,
            config_marks_enabled: w.config_marks_enabled,
            config_mark_kinds: w.config_mark_kinds,
            config_marks_decay_scale: w.config_marks_decay_scale,
            last_chalk_standing: w.last_chalk_standing,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub fn cost(&self) -> Result<MarksCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    fn validate(&self, c: MarksCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine marks version")?;
        owner::check(
            self.boundary == c.now && self.world.boundary == c.now,
            "Engine marks boundary disagreement",
        )?;
        self.world.validate(c)?;
        owner::id(self.player_id.as_str())?;
        owner::check(
            c.backbone.characters.contains_key(&self.player_id),
            "missing marks player binding",
        )?;
        validate_cache(&self.last_chalk_standing)
    }
}
/// A prior observation may lag public World mutations. Duplicate handles are
/// legitimate when registered normal places share a name. Saved ordering and
/// historical labels survive; only the intrinsic shape is checked here.
fn validate_cache(cache: &Option<EngineMessage>) -> Result<()> {
    let Some(EngineMessage::ChalkStanding { anchors, .. }) = cache else {
        return owner::check(cache.is_none(), "invalid chalk cache variant");
    };
    owner::check(
        anchors.len() <= owner::MAX_NAMES,
        "chalk cache anchor count",
    )?;
    for a in anchors {
        owner::text(&a.handle)?;
        owner::check(a.label.len() <= MAX_LABEL_BYTES, "chalk label byte limit")?;
        owner::check(
            !a.kinds.is_empty() && a.kinds.len() <= 3 && a.kinds.windows(2).all(|w| w[0] < w[1]),
            "chalk cache kind order/count",
        )?;
    }
    Ok(())
}
impl Admitted<EngineMarksDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: MarksCheckpointContext<'_>,
    ) -> Result<Admitted<EngineMarksCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineMarksCandidate { data: d })
        })
    }
}
impl EngineMarksCandidate {
    pub fn marks(&self) -> &crate::marks::Marks {
        &self.data.world.marks
    }
    pub fn marks_enabled(&self) -> bool {
        self.data.world.marks_enabled
    }
    pub fn mark_kinds(&self) -> crate::marks::MarkKindSwitches {
        self.data.world.mark_kinds
    }
    pub fn last_chalk_standing(&self) -> Option<&EngineMessage> {
        self.data.last_chalk_standing.as_ref()
    }
    pub fn counts(&self, c: MarksCheckpointContext<'_>) -> EngineMarksCounts {
        self.data.counts(c)
    }
}
const MAX_LABEL_BYTES: usize = crate::checkpoint::records::MAX_TEXT_BYTES + 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineMarksCounts {
    #[serde(flatten)]
    pub marks: owner::MarksCounts,
    pub chalk_cached: bool,
    pub cached_anchors: usize,
    pub cached_kinds: usize,
    pub cached_pen: bool,
}
impl EngineMarksDtoV1 {
    pub fn counts(&self, c: MarksCheckpointContext<'_>) -> EngineMarksCounts {
        let (cached_anchors, cached_kinds, cached_pen) = match &self.last_chalk_standing {
            Some(EngineMessage::ChalkStanding { pen, anchors }) => (
                anchors.len(),
                anchors.iter().map(|a| a.kinds.len()).sum(),
                *pen,
            ),
            _ => (0, 0, false),
        };
        EngineMarksCounts {
            marks: self.world.counts(c),
            chalk_cached: self.last_chalk_standing.is_some(),
            cached_anchors,
            cached_kinds,
            cached_pen,
        }
    }
}
#[cfg(test)]
mod tests;
