//! Existing law authority only. Whole World/Engine adoption and pending reply
//! composition remain later; no seeding, expiry, settlement or percept here.
use super::*;
use crate::{
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    clock::WorldTime,
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub mod context;
pub(crate) mod records;
pub use context::LawCheckpointContext;
const OWNER: &str = "law";
pub const MAX_NAMES: usize = 25_000;
/// Borrowed registry index validation (up to 25,000 entries) and bounded law
/// cross-reference scratch. No installed definition parser or singleton runs.
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
/// V1 format reserves 257 owner advances; this is a component horizon.
/// Full Engine/host catch-up and lifetime composition remain mandatory later.
pub const HANDLE_HEADROOM: u64 = crate::receipts::MAX_STEPS as u64 + 1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LawCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for LawCost {
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
        "invalid law identity",
    )
}
pub(crate) fn text(s: &str) -> Result<()> {
    check(
        s.len() <= crate::checkpoint::records::MAX_TEXT_BYTES,
        "law text byte limit",
    )
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<LawCost> {
    let cost = LawCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < cost.peak_bytes {
        r.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn context_for_export(
    c: LawCheckpointContext<'_>,
    r: &mut Reservation,
) -> Result<context::BindingV1> {
    r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
    if r.bytes() < aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES {
        r.resize(aggregate::INITIAL_BYTES + VALIDATION_WORKING_BYTES)?;
    }
    context::BindingV1::new(c)
}
pub(crate) fn validate(n: &Notices) -> Result<()> {
    check(n.live.len() <= NOTICES_MAX_LIVE, "live notice count")?;
    check(
        n.next_id <= u64::MAX - HANDLE_HEADROOM,
        "notice allocator lacks supported headroom",
    )?;
    check(
        n.live.windows(2).all(|w| w[0].id < w[1].id),
        "notice identities must be unique and ordered",
    )?;
    for row in &n.live {
        check(
            row.id > 0 && row.id <= n.next_id,
            "notice id/allocator disagreement",
        )?;
        text(&row.about)?;
        text(&row.deed)?;
        for t in [&row.place, &row.since].into_iter().flatten() {
            text(t)?;
        }
        id(row.raised_by.as_str())?;
        for a in [&row.accused, &row.wronged].into_iter().flatten() {
            id(a.as_str())?;
        }
        if let Some(i) = &row.taken {
            id(i.as_str())?;
        }
        if let Some(t) = row.raised_game_days {
            checkpoint::calendar(OWNER, t)?;
            checkpoint::calendar(OWNER, t + NOTICE_LIFE_GAME_DAYS)?;
        }
        if let Some(s) = &row.summons {
            id(s.by.as_str())?;
            if let Some(d) = s.due_game_days {
                checkpoint::calendar(OWNER, d)?;
            }
        }
        check(
            !row.warrant
                || row
                    .summons
                    .as_ref()
                    .is_some_and(|s| s.due_game_days.is_some()),
            "warrant lacks dated summons",
        )?;
        check(row.served.len() <= MAX_NAMES, "served notice count")?;
        check(
            row.accused.is_some() || row.served.is_empty(),
            "served notice lacks accused",
        )?;
        for who in &row.served {
            id(who.as_str())?;
            check(
                Some(who) != row.accused.as_ref(),
                "notice served to its own accused",
            )?;
        }
        // Served, accused/wronged/taken and raiser/summoner names are historical.
        // Warrant expiry, overdue summons and vanished people wait for their
        // ordinary owner pass, never a decoder mutation or current-truth test.
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct NoticesDtoV1 {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    pub(crate) notices: Notices,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoticesWire {
    version: u16,
    boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    pub(crate) notices: Notices,
}
#[derive(Serialize)]
struct NoticesView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    notices: &'a Notices,
}
#[derive(Debug)]
pub struct NoticesCandidate {
    pub(crate) data: NoticesDtoV1,
}
impl Notices {
    pub fn export_checkpoint(
        &self,
        c: LawCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<NoticesDtoV1>> {
        let context = context_for_export(c, &mut r)?;
        prepare(
            &NoticesView {
                version: 1,
                boundary: c.now,
                context: &context,
                sampled_time: c.backbone.current_time,
                notices: self,
            },
            &mut r,
        )?;
        context::boundary(1, c.now, &context, c.backbone.current_time, c)?;
        validate(self)?;
        Ok(Admitted::new(
            NoticesDtoV1 {
                version: 1,
                boundary: c.now,
                context,
                sampled_time: c.backbone.current_time,
                notices: self.clone(),
            },
            r,
        ))
    }
}
impl NoticesDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: NoticesWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            notices: w.notices,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: LawCheckpointContext<'_>) -> Result<()> {
        context::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate(&self.notices)?;
        Ok(())
    }
    pub fn cost(&self) -> Result<LawCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
}
impl Admitted<NoticesDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(self, c: LawCheckpointContext<'_>) -> Result<Admitted<NoticesCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(NoticesCandidate { data: d })
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldLawDtoV1 {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    pub(crate) notices: Notices,
    #[serde(with = "crate::custody::checkpoint::records::CustodyV1")]
    pub(crate) custody: crate::custody::Custody,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    pub(crate) boundary: LogicalTime,
    context: context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    pub(crate) notices: Notices,
    #[serde(with = "crate::custody::checkpoint::records::CustodyV1")]
    pub(crate) custody: crate::custody::Custody,
}
#[derive(Serialize)]
pub(crate) struct WorldView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: &'a context::BindingV1,
    #[serde(with = "crate::checkpoint::records::world_time::option")]
    sampled_time: Option<WorldTime>,
    #[serde(with = "records::NoticesV1")]
    notices: &'a Notices,
    #[serde(with = "crate::custody::checkpoint::records::CustodyV1")]
    custody: &'a crate::custody::Custody,
}
#[derive(Debug)]
pub struct WorldLawCandidate {
    pub(crate) data: WorldLawDtoV1,
}
impl World {
    pub fn export_law_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldLawDtoV1>> {
        let c = LawCheckpointContext::from_world(self, now);
        let context = context_for_export(c, &mut r)?;
        prepare(
            &WorldView {
                version: 1,
                boundary: c.now,
                context: &context,
                sampled_time: c.backbone.current_time,
                notices: &self.notices,
                custody: &self.custody,
            },
            &mut r,
        )?;
        context::boundary(1, c.now, &context, c.backbone.current_time, c)?;
        validate(&self.notices)?;
        crate::custody::checkpoint::validate(&self.custody, c)?;
        Ok(Admitted::new(
            WorldLawDtoV1 {
                version: 1,
                boundary: c.now,
                context,
                sampled_time: c.backbone.current_time,
                notices: self.notices.clone(),
                custody: self.custody.clone(),
            },
            r,
        ))
    }
}
impl WorldLawDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            notices: w.notices,
            custody: w.custody,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub(crate) fn validate(&self, c: LawCheckpointContext<'_>) -> Result<()> {
        context::boundary(
            self.version,
            self.boundary,
            &self.context,
            self.sampled_time,
            c,
        )?;
        validate(&self.notices)?;
        crate::custody::checkpoint::validate(&self.custody, c)?;
        Ok(())
    }
    pub fn cost(&self) -> Result<LawCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
}
impl Admitted<WorldLawDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<WorldLawCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(WorldLawCandidate { data: d })
        })
    }
}

impl NoticesCandidate {
    pub fn notices(&self) -> &Notices {
        &self.data.notices
    }
}
impl WorldLawCandidate {
    pub fn notices(&self) -> &Notices {
        &self.data.notices
    }
    pub fn custody(&self) -> &crate::custody::Custody {
        &self.data.custody
    }
}

impl<'a> WorldView<'a> {
    pub(crate) fn new(w: &'a World, now: LogicalTime, context: &'a context::BindingV1) -> Self {
        Self {
            version: 1,
            boundary: now,
            context,
            sampled_time: w.current_time,
            notices: &w.notices,
            custody: &w.custody,
        }
    }
}
pub(crate) struct WorldLawV1;
impl WorldLawV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &WorldLawDtoV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<WorldLawDtoV1, D::Error> {
        let w = WorldWire::deserialize(d)?;
        Ok(WorldLawDtoV1 {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            sampled_time: w.sampled_time,
            notices: w.notices,
            custody: w.custody,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LawCounts {
    pub characters: usize,
    pub notices: usize,
    pub served_notices: usize,
    pub served_pairs: usize,
    pub summons: usize,
    pub dated_unissued_summons: usize,
    pub undated_summons: usize,
    pub warrants: usize,
    pub hearsay: usize,
    pub custody_records: usize,
    pub arrests: usize,
    pub authored: usize,
    pub holders: usize,
    pub closing: usize,
    pub committed: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LawHistoryCounts {
    pub historical_notice_links: usize,
    pub historical_officers: usize,
}
impl WorldLawDtoV1 {
    pub fn counts(&self, c: LawCheckpointContext<'_>) -> LawCounts {
        let n = &self.notices;
        let records = || self.custody.iter().map(|(_, r)| r);
        LawCounts {
            characters: c.backbone.characters.len(),
            notices: n.live.len(),
            served_notices: n.live.iter().filter(|n| !n.served.is_empty()).count(),
            served_pairs: n.live.iter().map(|n| n.served.len()).sum(),
            summons: n.live.iter().filter(|n| n.summons.is_some()).count(),
            dated_unissued_summons: n
                .live
                .iter()
                .filter(|n| {
                    !n.warrant
                        && n.summons
                            .as_ref()
                            .is_some_and(|s| s.due_game_days.is_some())
                })
                .count(),
            undated_summons: n
                .live
                .iter()
                .filter(|n| {
                    n.summons
                        .as_ref()
                        .is_some_and(|s| s.due_game_days.is_none())
                })
                .count(),
            warrants: n.live.iter().filter(|n| n.warrant).count(),
            hearsay: n.live.iter().filter(|n| n.hearsay).count(),
            custody_records: self.custody.iter().count(),
            arrests: self.custody.arrest_count(),
            authored: records().filter(|r| r.authored).count(),
            holders: records().map(|r| r.holders.len()).sum(),
            closing: records().filter(|r| r.closing).count(),
            committed: records()
                .filter(|r| r.state == crate::custody::Confinement::Committed)
                .count(),
        }
    }
    pub fn history_counts(&self, c: LawCheckpointContext<'_>) -> LawHistoryCounts {
        LawHistoryCounts {
            historical_notice_links: self
                .custody
                .iter()
                .filter(|(_, r)| r.notice_id.is_some_and(|n| self.notices.get(n).is_none()))
                .count(),
            historical_officers: self
                .custody
                .iter()
                .filter(|(_, r)| {
                    r.state == crate::custody::Confinement::Committed
                        && r.officer
                            .as_ref()
                            .is_some_and(|a| !c.backbone.characters.contains_key(a))
                })
                .count(),
        }
    }
}
impl WorldLawCandidate {
    pub fn counts(&self, c: LawCheckpointContext<'_>) -> LawCounts {
        self.data.counts(c)
    }
    pub fn history_counts(&self, c: LawCheckpointContext<'_>) -> LawHistoryCounts {
        self.data.history_counts(c)
    }
}

#[cfg(test)]
pub(crate) mod tests;
