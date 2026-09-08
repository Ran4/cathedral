//! Pollen cadence and its observable caches only. Other Engine owners and
//! complete-poll/host capture composition remain mandatory later components.
use super::*;
use crate::{
    checkpoint::{self, Admitted, Reservation, Result, aggregate},
    knowledge::{
        self,
        checkpoint::{
            self as owner, KnowledgeCheckpointContext, KnowledgeCost, KnowledgeCounts,
            WorldKnowledgeDtoV1,
        },
    },
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod records;
const OWNER: &str = "engine_knowledge";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineKnowledgeCounts {
    #[serde(flatten)]
    pub knowledge: KnowledgeCounts,
    pub door_timers: usize,
    pub journal_entries: usize,
    pub journal_cached: bool,
    pub ward_heat_rows: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KnowledgeHistoryCounts {
    pub historical_receipts: usize,
    pub historical_seated_keys: usize,
    pub offered_occasions: usize,
}
mod doors {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &BTreeMap<(ActorId, ActorId), f64>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|((a, b), t)| (a, b, t)))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<BTreeMap<(ActorId, ActorId), f64>, D::Error> {
        let rows = Vec::<(ActorId, ActorId, f64)>::deserialize(d)?;
        let mut out = BTreeMap::new();
        for (a, b, t) in rows {
            if out.insert((a, b), t).is_some() {
                return Err(serde::de::Error::custom("duplicate door timer pair"));
            }
        }
        Ok(out)
    }
}
#[derive(Debug, Serialize)]
pub struct EngineKnowledgeDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldKnowledgeV1")]
    world: WorldKnowledgeDtoV1,
    player_id: ActorId,
    next_stage_hop_at: f64,
    next_player_pollen_game_days: checkpoint::CalendarAnchorV1,
    #[serde(with = "doors")]
    door_shut_until: BTreeMap<(ActorId, ActorId), f64>,
    #[serde(with = "records::journal::option")]
    last_journal: Option<EngineMessage>,
    last_journal_receipts: u64,
    last_journal_at: checkpoint::LogicalAnchorV1,
    #[serde(with = "records::ward_heat::option")]
    last_ward_heat: Option<EngineMessage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldKnowledgeV1")]
    world: WorldKnowledgeDtoV1,
    player_id: ActorId,
    next_stage_hop_at: f64,
    next_player_pollen_game_days: checkpoint::CalendarAnchorV1,
    #[serde(with = "doors")]
    door_shut_until: BTreeMap<(ActorId, ActorId), f64>,
    #[serde(with = "records::journal::option")]
    last_journal: Option<EngineMessage>,
    last_journal_receipts: u64,
    last_journal_at: checkpoint::LogicalAnchorV1,
    #[serde(with = "records::ward_heat::option")]
    last_ward_heat: Option<EngineMessage>,
}
#[derive(Serialize)]
struct View<'a, W: Serialize> {
    version: u16,
    boundary: LogicalTime,
    world: W,
    player_id: &'a ActorId,
    next_stage_hop_at: f64,
    next_player_pollen_game_days: checkpoint::CalendarAnchorV1,
    #[serde(with = "doors")]
    door_shut_until: &'a BTreeMap<(ActorId, ActorId), f64>,
    #[serde(with = "records::journal::option")]
    last_journal: &'a Option<EngineMessage>,
    last_journal_receipts: u64,
    last_journal_at: checkpoint::LogicalAnchorV1,
    #[serde(with = "records::ward_heat::option")]
    last_ward_heat: &'a Option<EngineMessage>,
}
impl<'a, W: Serialize> View<'a, W> {
    fn new(e: &'a Engine, now: LogicalTime, world: W) -> Result<Self> {
        Ok(Self {
            version: 1,
            boundary: now,
            world,
            player_id: &e.config.player_id,
            next_stage_hop_at: e.next_stage_hop_at,
            next_player_pollen_game_days: checkpoint::CalendarAnchorV1::from_legacy(
                e.next_player_pollen_game_days,
            )?,
            door_shut_until: &e.door_shut_until,
            last_journal: &e.last_journal,
            last_journal_receipts: e.last_journal_receipts,
            last_journal_at: checkpoint::LogicalAnchorV1::from_legacy(e.last_journal_at)?,
            last_ward_heat: &e.last_ward_heat,
        })
    }
}
#[derive(Debug)]
pub struct EngineKnowledgeCandidate {
    data: EngineKnowledgeDtoV1,
}
impl Engine {
    pub fn checkpoint_knowledge_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<KnowledgeCost>> {
        let c = KnowledgeCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        let world = owner::WorldView::new(&self.world, now, &binding);
        let view = View::new(self, now, world)?;
        let cost = owner::prepare(&view, &mut r)?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_knowledge_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineKnowledgeDtoV1>> {
        // Admit the entire borrowed shape before either owner is extracted.
        // The nested world exporter preserves the larger attached charge.
        let c = KnowledgeCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding))?,
            &mut r,
        )?;
        self.world
            .export_knowledge_checkpoint(now, r)?
            .try_map(|world, _| {
                let d = EngineKnowledgeDtoV1 {
                    version: 1,
                    boundary: now,
                    world,
                    player_id: self.config.player_id.clone(),
                    next_stage_hop_at: self.next_stage_hop_at,
                    next_player_pollen_game_days: checkpoint::CalendarAnchorV1::from_legacy(
                        self.next_player_pollen_game_days,
                    )?,
                    door_shut_until: self.door_shut_until.clone(),
                    last_journal: self.last_journal.clone(),
                    last_journal_receipts: self.last_journal_receipts,
                    last_journal_at: checkpoint::LogicalAnchorV1::from_legacy(
                        self.last_journal_at,
                    )?,
                    last_ward_heat: self.last_ward_heat.clone(),
                };
                d.validate(KnowledgeCheckpointContext::from_world(&self.world, now))?;
                Ok(d)
            })
    }
}
impl EngineKnowledgeDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, owner::VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            player_id: w.player_id,
            next_stage_hop_at: w.next_stage_hop_at,
            next_player_pollen_game_days: w.next_player_pollen_game_days,
            door_shut_until: w.door_shut_until,
            last_journal: w.last_journal,
            last_journal_receipts: w.last_journal_receipts,
            last_journal_at: w.last_journal_at,
            last_ward_heat: w.last_ward_heat,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub fn cost(&self) -> Result<KnowledgeCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: KnowledgeCheckpointContext<'_>) -> EngineKnowledgeCounts {
        EngineKnowledgeCounts {
            knowledge: self.world.counts(c),
            door_timers: self.door_shut_until.len(),
            journal_entries: match &self.last_journal {
                Some(EngineMessage::Journal { entries, .. }) => entries.len(),
                _ => 0,
            },
            journal_cached: self.last_journal.is_some(),
            ward_heat_rows: match &self.last_ward_heat {
                Some(EngineMessage::WardHeat { wards }) => wards.len(),
                _ => 0,
            },
        }
    }
    pub fn history_counts(&self) -> KnowledgeHistoryCounts {
        let (historical_receipts, historical_seated_keys, offered_occasions) =
            owner::history_counts(&self.world.knowledge);
        KnowledgeHistoryCounts {
            historical_receipts,
            historical_seated_keys,
            offered_occasions,
        }
    }
    fn validate(&self, c: KnowledgeCheckpointContext<'_>) -> Result<()> {
        use owner::{check, id, text};
        check(self.version == 1, "unsupported Engine knowledge version")?;
        check(
            self.boundary == c.now && self.world.boundary == c.now,
            "Engine knowledge boundary disagreement",
        )?;
        self.world.validate(c)?;
        knowledge::pollen::checkpoint::validate(&self.world.area_adjacency, c.areas, true)?;
        id(self.player_id.as_str())?;
        check(
            c.backbone.characters.contains_key(&self.player_id),
            "missing knowledge player binding",
        )?;
        checkpoint::logical(OWNER, self.next_stage_hop_at)?;
        self.next_player_pollen_game_days.validate()?;
        self.last_journal_at.validate()?;
        check(
            self.last_journal_at.legacy() <= c.now.seconds(),
            "journal publication anchor is in the future",
        )?;
        let tolerance = 16.0 * f64::EPSILON * c.now.seconds().abs().max(1.0);
        check(
            self.next_stage_hop_at <= c.now.seconds() + knowledge::STAGE_HOP_SECONDS + tolerance,
            "stage hop deadline beyond cadence",
        )?;
        if let Some(time) = c.backbone.current_time {
            let days = time.game_days();
            let tolerance = 16.0 * f64::EPSILON * days.abs().max(1.0);
            check(
                self.next_player_pollen_game_days.legacy()
                    <= days + knowledge::PLAYER_POLL_GAME_MINUTES / 1440.0 + tolerance,
                "player pollen deadline beyond cadence",
            )?;
        }
        check(
            self.door_shut_until.len() <= owner::MAX_NAMES,
            "door timer count",
        )?;
        for ((a, b), at) in &self.door_shut_until {
            id(a.as_str())?;
            id(b.as_str())?;
            check(
                a == &self.player_id,
                "door timer caller disagrees with player",
            )?;
            checkpoint::calendar(OWNER, *at)?;
        }
        // Door pruning is Stage-only; expired/absent names survive other modes.
        match &self.last_journal {
            None => check(
                matches!(self.last_journal_at, checkpoint::LogicalAnchorV1::Never)
                    && self.last_journal_receipts == 0,
                "virgin journal cache disagreement",
            )?,
            Some(EngineMessage::Journal { entries, standing }) => {
                check(
                    !matches!(self.last_journal_at, checkpoint::LogicalAnchorV1::Never),
                    "journal cache lacks publication anchor",
                )?;
                check(
                    entries.len() <= knowledge::JOURNAL_ENTRIES_MAX
                        && standing.len() <= knowledge::FACTS_MAX_LIVE,
                    "journal cache count",
                )?;
                for e in entries {
                    text(&e.word)?;
                    text(&e.when)?;
                    for s in [&e.from, &e.place].into_iter().flatten() {
                        text(s)?;
                    }
                    check(
                        e.tellings <= knowledge::PLAYER_RECEIPTS_MAX as u16 + 1 && e.wards <= 8,
                        "journal cached count",
                    )?;
                }
                for s in standing {
                    text(s)?;
                }
            }
            _ => return Err(owner::error("invalid journal cache variant")),
        }
        // Journal text/attribution/standing can lag current World until its
        // one-second throttle permits publication. Preserve it, never refresh.
        match &self.last_ward_heat {
            None => {}
            Some(EngineMessage::WardHeat { wards }) => {
                check(
                    wards.len() == crate::lore::PlanningWard::ALL.len(),
                    "ward heat cache count",
                )?;
                let centers = knowledge::pollen::checkpoint::centroids()?;
                for (row, expected) in wards.iter().zip(crate::lore::PlanningWard::ALL) {
                    check(
                        row.ward == expected && row.label == crate::prompt::ward_label(expected),
                        "ward heat cache identity",
                    )?;
                    crate::character::checkpoint::point(row.at)?;
                    let expected_at = centers.get(&expected).copied().unwrap_or(Vec3::ZERO);
                    check(
                        row.at.to_array().map(f64::to_bits)
                            == expected_at.to_array().map(f64::to_bits),
                        "ward heat centroid disagreement",
                    )?;
                    check(
                        row.heat_pct <= 100
                            && usize::from(row.words) <= knowledge::AIR_PER_WARD_MAX,
                        "ward heat cache numeric state",
                    )?;
                }
            }
            _ => return Err(owner::error("invalid ward heat cache variant")),
        }
        Ok(())
    }
}
impl Admitted<EngineKnowledgeDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: KnowledgeCheckpointContext<'_>,
    ) -> Result<Admitted<EngineKnowledgeCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineKnowledgeCandidate { data: d })
        })
    }
}
impl EngineKnowledgeCandidate {
    pub fn counts(&self, c: KnowledgeCheckpointContext<'_>) -> EngineKnowledgeCounts {
        self.data.counts(c)
    }
    pub fn knowledge(&self) -> &knowledge::Knowledge {
        &self.data.world.knowledge
    }
    pub fn history_counts(&self) -> KnowledgeHistoryCounts {
        self.data.history_counts()
    }
}

#[cfg(test)]
mod tests;
