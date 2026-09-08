//! Existing law authority and exact publication cache, with no partial adoption.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate},
    notices::checkpoint::{self as owner, LawCheckpointContext, LawCost, WorldLawDtoV1},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod records;
const OWNER: &str = "engine_law";
#[derive(Debug, Serialize)]
pub struct EngineLawDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldLawV1")]
    world: WorldLawDtoV1,
    player_id: ActorId,
    #[serde(with = "records::standing::option")]
    last_law_standing: Option<EngineMessage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldLawV1")]
    world: WorldLawDtoV1,
    player_id: ActorId,
    #[serde(with = "records::standing::option")]
    last_law_standing: Option<EngineMessage>,
}
#[derive(Serialize)]
struct View<'a, W: Serialize> {
    version: u16,
    boundary: LogicalTime,
    world: W,
    player_id: &'a ActorId,
    #[serde(with = "records::standing::option")]
    last_law_standing: &'a Option<EngineMessage>,
}
impl<'a, W: Serialize> View<'a, W> {
    fn new(e: &'a Engine, now: LogicalTime, world: W) -> Self {
        Self {
            version: 1,
            boundary: now,
            world,
            player_id: &e.config.player_id,
            last_law_standing: &e.last_law_standing,
        }
    }
}
#[derive(Debug)]
pub struct EngineLawCandidate {
    data: EngineLawDtoV1,
}
impl Engine {
    pub fn checkpoint_law_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<LawCost>> {
        let c = LawCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        let cost = owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding)),
            &mut r,
        )?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_law_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineLawDtoV1>> {
        let c = LawCheckpointContext::from_world(&self.world, now);
        let binding = owner::context_for_export(c, &mut r)?;
        owner::prepare(
            &View::new(self, now, owner::WorldView::new(&self.world, now, &binding)),
            &mut r,
        )?;
        self.world
            .export_law_checkpoint(now, r)?
            .try_map(|world, _| {
                let d = EngineLawDtoV1 {
                    version: 1,
                    boundary: now,
                    world,
                    player_id: self.config.player_id.clone(),
                    last_law_standing: self.last_law_standing.clone(),
                };
                d.validate(c)?;
                Ok(d)
            })
    }
}
impl EngineLawDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, owner::VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            player_id: w.player_id,
            last_law_standing: w.last_law_standing,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    pub fn cost(&self) -> Result<LawCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    fn validate(&self, c: LawCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine law version")?;
        owner::check(
            self.boundary == c.now && self.world.boundary == c.now,
            "Engine law boundary disagreement",
        )?;
        self.world.validate(c)?;
        owner::id(self.player_id.as_str())?;
        owner::check(
            c.backbone.characters.contains_key(&self.player_id),
            "missing law player binding",
        )?;
        validate_cache(&self.last_law_standing)
    }
}
/// A cached publication is a historical observation, including names, positions
/// and notice links. Public World mutations can precede the next poll/publisher.
/// Validate its own immutable geometry/shape; never refresh or equate it to the
/// latest owner records here. Full host initial republish is a separate contract.
fn validate_cache(cache: &Option<EngineMessage>) -> Result<()> {
    use owner::{check, id, text};
    let Some(message) = cache else { return Ok(()) };
    let EngineMessage::LawStanding { notices, custody } = message else {
        return Err(owner::error("invalid law cache variant"));
    };
    check(
        notices.len() <= notices::NOTICES_MAX_LIVE,
        "law cache notice count",
    )?;
    check(
        notices.windows(2).all(|w| {
            w[0].rung > w[1].rung || (w[0].rung == w[1].rung && w[0].notice_id > w[1].notice_id)
        }),
        "law cache notice order",
    )?;
    let mut seen = BTreeSet::new();
    for n in notices {
        check(
            n.notice_id > 0
                && n.notice_id <= u64::MAX - owner::HANDLE_HEADROOM
                && seen.insert(n.notice_id),
            "law cache notice identity",
        )?;
        check(
            n.line.len() <= MAX_NOTICE_LINE_BYTES,
            "law cached line byte limit",
        )?;
        check(
            matches!(
                n.clears_when.as_str(),
                "nobody saw this one — answer it, or find who did and say so"
                    | "give back what was taken, or satisfy the law"
                    | "make it right with the one you wronged, or satisfy the law"
                    | "only the law can end this one — go and answer for it"
            ),
            "law cache settlement wording",
        )?;
    }
    if let Some(c) = custody {
        check(
            c.holder_ids.len() <= owner::MAX_NAMES,
            "law cache holder count",
        )?;
        let mut seen = BTreeSet::new();
        for h in &c.holder_ids {
            id(h.as_str())?;
            check(seen.insert(h), "duplicate law cache holder")?;
        }
        if let Some(a) = &c.officer_id {
            id(a.as_str())?;
        }
        text(&c.officer_name)?;
        text(&c.station_name)?;
        for s in [&c.release_office, &c.booked_as].into_iter().flatten() {
            text(s)?;
        }
        crate::character::checkpoint::point(c.anchor_m)?;
        check(
            c.leash_m.to_bits() == custody::CUSTODY_LEASH_M.to_bits()
                && c.tether_m.to_bits() == custody::CUSTODY_TETHER_M.to_bits()
                && c.reach_m.to_bits() == custody::CUSTODY_REACH_M.to_bits()
                && c.fee_sparks == custody::GAOL_FEE_SPARKS,
            "law cache immutable geometry/fee disagreement",
        )?;
        let minimum_strain = match c.holder_ids.len() {
            0 => 0.0,
            1 => custody::STRAIN_BASE_SECONDS,
            2 => 7.0 * custody::STRAIN_BASE_SECONDS,
            _ => 20.0 * custody::STRAIN_BASE_SECONDS,
        };
        check(
            c.held == !c.holder_ids.is_empty()
                && c.strain_seconds.is_finite()
                && c.strain_seconds >= minimum_strain
                && (c.held || c.strain_seconds == 0.0),
            "law cache grip disagreement",
        )?;
        if let Some(label) = &c.release_office {
            check(
                c.committed && crate::clock::Office::ALL.iter().any(|o| o.label() == label),
                "law cache office label",
            )?;
        }
    }
    Ok(())
}
impl Admitted<EngineLawDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: LawCheckpointContext<'_>,
    ) -> Result<Admitted<EngineLawCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineLawCandidate { data: d })
        })
    }
}
impl EngineLawCandidate {
    pub fn notices(&self) -> &notices::Notices {
        &self.data.world.notices
    }
    pub fn custody(&self) -> &custody::Custody {
        &self.data.world.custody
    }
    pub fn last_law_standing(&self) -> Option<&EngineMessage> {
        self.data.last_law_standing.as_ref()
    }
}

/// Four bounded notice prose fields plus the fixed rung/attribution separators.
const MAX_NOTICE_LINE_BYTES: usize = 4 * crate::checkpoint::records::MAX_TEXT_BYTES + 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineLawCounts {
    #[serde(flatten)]
    pub law: owner::LawCounts,
    pub law_cached: bool,
    pub cached_notices: usize,
    pub cached_custody: bool,
}
impl EngineLawDtoV1 {
    pub fn counts(&self, c: LawCheckpointContext<'_>) -> EngineLawCounts {
        let (cached_notices, cached_custody) = match &self.last_law_standing {
            Some(EngineMessage::LawStanding { notices, custody }) => {
                (notices.len(), custody.is_some())
            }
            _ => (0, false),
        };
        EngineLawCounts {
            law: self.world.counts(c),
            law_cached: self.last_law_standing.is_some(),
            cached_notices,
            cached_custody,
        }
    }
    pub fn history_counts(&self, c: LawCheckpointContext<'_>) -> owner::LawHistoryCounts {
        self.world.history_counts(c)
    }
}
impl EngineLawCandidate {
    pub fn counts(&self, c: LawCheckpointContext<'_>) -> EngineLawCounts {
        self.data.counts(c)
    }
    pub fn history_counts(&self, c: LawCheckpointContext<'_>) -> owner::LawHistoryCounts {
        self.data.history_counts(c)
    }
}

#[cfg(test)]
mod tests;
