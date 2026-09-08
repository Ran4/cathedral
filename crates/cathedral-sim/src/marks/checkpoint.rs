//! Exact existing chalk authority. No constructor, cleanup, seeding or partial
//! World adoption is exposed; the complete envelope remains a later owner.
use super::*;
use crate::{
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    clock::WorldTime,
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub mod context;
pub(crate) mod records;
pub use context::MarksCheckpointContext;
const OWNER: &str = "marks";
pub const MAX_NAMES: usize = 25_000;
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
pub const HANDLE_HEADROOM: u64 = crate::receipts::MAX_STEPS as u64 + 1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MarksCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for MarksCost {
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
    check(
        s.len() <= 4 * crate::MAX_ID_CHARS && crate::ids::is_valid_id(s),
        "invalid marks identity",
    )
}
pub(crate) fn text(s: &str) -> Result<()> {
    check(
        s.len() <= crate::checkpoint::records::MAX_TEXT_BYTES,
        "marks text byte limit",
    )
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<MarksCost> {
    let cost = MarksCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn context_for_export(
    c: MarksCheckpointContext<'_>,
    r: &mut Reservation,
) -> Result<context::BindingV1> {
    r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    if r.bytes() < aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES {
        r.resize(aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES)?;
    }
    context::BindingV1::new(c)
}
fn validate(m: &Marks) -> Result<()> {
    check(m.live.len() <= MARKS_MAX, "live mark count")?;
    check(
        m.next_id <= u64::MAX - HANDLE_HEADROOM,
        "mark allocator lacks supported headroom",
    )?;
    checkpoint::CalendarAnchorV1::from_legacy(m.last_sweep_game_days)?;
    checkpoint::CalendarAnchorV1::from_legacy(m.last_beat_game_days)?;
    // decay_scale is exact IEEE bits, including externally reachable invalid
    // configs. sweep's existing fallback is the only place that interprets it.
    for (key, row) in &m.live {
        check(
            key.0 > 0 && key.0 <= m.next_id,
            "mark id/allocator disagreement",
        )?;
        match &row.anchor {
            MarkAnchor::Household(a) => id(a.as_str())?,
            MarkAnchor::Place(p) => text(p)?,
        }
        for a in [&row.about, &row.author].into_iter().flatten() {
            id(a.as_str())?;
        }
        checkpoint::calendar(OWNER, row.drawn_game_days)?;
        checkpoint::calendar(OWNER, row.last_decayed_game_days)?;
        check(
            row.strength.is_finite() && (0.0..=1.0).contains(&row.strength),
            "mark strength range",
        )?;
        check(
            row.strokes <= TALLY_STROKES_MAX
                && (row.kind == MarkKind::WellTally || row.strokes == 1),
            "mark stroke range",
        )?;
        // Public mutability, catalog changes and removed anchors can leave
        // historical rows until ordinary gated cleanup. Do not require current
        // actor existence, anchor resolution, catalog acceptance, unique pairs,
        // or drawn <= decayed <= sampled-time ordering here.
    }
    Ok(())
}
#[derive(Debug, Serialize)]
pub struct WorldMarksDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::MarksV1")]
    pub(crate) marks: Marks,
    pub(crate) marks_enabled: bool,
    #[serde(with = "records::SwitchesV1")]
    pub(crate) mark_kinds: MarkKindSwitches,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::MarksV1")]
    marks: Marks,
    marks_enabled: bool,
    #[serde(with = "records::SwitchesV1")]
    mark_kinds: MarkKindSwitches,
}
impl From<WorldWire> for WorldMarksDtoV1 {
    fn from(w: WorldWire) -> Self {
        Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            marks: w.marks,
            marks_enabled: w.marks_enabled,
            mark_kinds: w.mark_kinds,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct WorldView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::MarksV1")]
    marks: &'a Marks,
    marks_enabled: bool,
    #[serde(with = "records::SwitchesV1")]
    mark_kinds: MarkKindSwitches,
}
impl<'a> WorldView<'a> {
    pub(crate) fn new(w: &'a World, now: LogicalTime, context: &'a context::BindingV1) -> Self {
        Self {
            version: 1,
            boundary: now,
            context,
            sampled_time: w.current_time,
            marks: &w.marks,
            marks_enabled: w.marks_enabled,
            mark_kinds: w.mark_kinds,
        }
    }
}
#[derive(Debug)]
pub struct WorldMarksCandidate {
    pub(crate) data: WorldMarksDtoV1,
}
impl World {
    pub fn export_marks_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldMarksDtoV1>> {
        let c = MarksCheckpointContext::from_world(self, now);
        let context = context_for_export(c, &mut r)?;
        prepare(&WorldView::new(self, now, &context), &mut r)?;
        context::boundary(1, now, &context, self.current_time, c)?;
        validate(&self.marks)?;
        Ok(Admitted::new(
            WorldMarksDtoV1 {
                version: 1,
                boundary: now,
                context,
                sampled_time: self.current_time,
                marks: self.marks.clone(),
                marks_enabled: self.marks_enabled,
                mark_kinds: self.mark_kinds,
            },
            r,
        ))
    }
}
impl WorldMarksDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: MarksCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self::from(w);
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: MarksCheckpointContext<'_>) -> Result<()> {
        context::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate(&self.marks)
    }
    pub fn cost(&self) -> Result<MarksCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: MarksCheckpointContext<'_>) -> MarksCounts {
        MarksCounts {
            characters: c.backbone.characters.len(),
            marks: self.marks.len(),
            crosses: self
                .marks
                .iter()
                .filter(|(_, m)| m.kind == MarkKind::ChalkCross)
                .count(),
            tallies: self
                .marks
                .iter()
                .filter(|(_, m)| m.kind == MarkKind::WellTally)
                .count(),
            ward_signs: self
                .marks
                .iter()
                .filter(|(_, m)| m.kind == MarkKind::WardSign)
                .count(),
            households: self
                .marks
                .iter()
                .filter(|(_, m)| matches!(m.anchor, MarkAnchor::Household(_)))
                .count(),
            places: self
                .marks
                .iter()
                .filter(|(_, m)| matches!(m.anchor, MarkAnchor::Place(_)))
                .count(),
            faint: self
                .marks
                .iter()
                .filter(|(_, m)| c.catalog.is_faint(m))
                .count(),
            historical_authors: self
                .marks
                .iter()
                .filter(|(_, m)| {
                    m.author
                        .as_ref()
                        .is_some_and(|a| !c.backbone.characters.contains_key(a))
                })
                .count(),
            historical_subjects: self
                .marks
                .iter()
                .filter(|(_, m)| {
                    m.about
                        .as_ref()
                        .is_some_and(|a| !c.backbone.characters.contains_key(a))
                })
                .count(),
            sweep_taken: self.marks.last_sweep_game_days.is_finite(),
            beat_taken: self.marks.last_beat_game_days.is_finite(),
        }
    }
}
impl Admitted<WorldMarksDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: MarksCheckpointContext<'_>,
    ) -> Result<Admitted<WorldMarksCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(WorldMarksCandidate { data: d })
        })
    }
}
impl WorldMarksCandidate {
    pub fn marks(&self) -> &Marks {
        &self.data.marks
    }
    pub fn marks_enabled(&self) -> bool {
        self.data.marks_enabled
    }
    pub fn mark_kinds(&self) -> MarkKindSwitches {
        self.data.mark_kinds
    }
    pub fn counts(&self, c: MarksCheckpointContext<'_>) -> MarksCounts {
        self.data.counts(c)
    }
}
pub(crate) struct WorldMarksV1;
impl WorldMarksV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &WorldMarksDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WorldMarksDtoV1, D::Error> {
        WorldWire::deserialize(d).map(Into::into)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MarksCounts {
    pub characters: usize,
    pub marks: usize,
    pub crosses: usize,
    pub tallies: usize,
    pub ward_signs: usize,
    pub households: usize,
    pub places: usize,
    pub faint: usize,
    pub historical_authors: usize,
    pub historical_subjects: usize,
    pub sweep_taken: bool,
    pub beat_taken: bool,
}
#[cfg(test)]
pub(crate) mod tests;
