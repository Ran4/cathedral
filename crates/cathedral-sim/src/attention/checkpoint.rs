//! Exact private social authority; candidates are read-only and never installed in production.
use super::*;
pub use crate::checkpoint::social::SocialCost;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate, social},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub(crate) mod records;
use records::*;
pub(crate) fn validate_warm(v: &WarmExchanges) -> Result<()> {
    social::check(v.pairs.len() <= social::MAX_WARM_PAIRS, "warm pair count")?;
    for ((a, b), at) in &v.pairs {
        social::id(a)?;
        social::id(b)?;
        social::check(a < b, "noncanonical warm pair")?;
        social::anchor(*at)?;
    }
    Ok(())
}
pub(crate) fn validate_novelty(v: &Novelty) -> Result<()> {
    social::check(
        v.last_told.len() <= social::MAX_NOVELTY_MEMORIES,
        "novelty memory count",
    )?;
    for (a, m) in &v.last_told {
        social::id(a)?;
        social::anchor(m.touched_at)?;
    }
    // context and visit are opaque u64s. No context rehash or visit-as-time gate.
    Ok(())
}
pub(crate) fn counts(w: &WarmExchanges, n: &Novelty) -> (usize, usize, usize) {
    (
        w.pairs.len(),
        n.last_told.len(),
        n.last_told.values().filter(|m| m.context.is_some()).count(),
    )
}

#[derive(Debug, Serialize)]
pub struct WarmExchangesDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "WarmExchangesV1")]
    state: WarmExchanges,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarmWire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "WarmExchangesV1")]
    state: WarmExchanges,
}
#[derive(Serialize)]
struct WarmView<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "WarmExchangesV1")]
    state: &'a WarmExchanges,
}
#[derive(Debug)]
pub struct WarmExchangesCandidate {
    data: WarmExchangesDtoV1,
}
impl WarmExchanges {
    pub fn export_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WarmExchangesDtoV1>> {
        social::prepare(
            &WarmView {
                version: 1,
                boundary: now,
                state: self,
            },
            &mut r,
        )?;
        crate::checkpoint::logical("social", now.seconds())?;
        validate_warm(self)?;
        Ok(Admitted::new(
            WarmExchangesDtoV1 {
                version: 1,
                boundary: now,
                state: self.clone(),
            },
            r,
        ))
    }
}
impl WarmExchangesDtoV1 {
    pub fn decode(bytes: &[u8], mut r: Reservation, now: LogicalTime) -> Result<Admitted<Self>> {
        let w: WarmWire = aggregate::decode_with_working(
            bytes,
            "social",
            &mut r,
            social::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            state: w.state,
        };
        d.validate(now)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, now: LogicalTime) -> Result<()> {
        crate::checkpoint::logical("social", now.seconds())?;
        social::check(self.version == 1, "unsupported social version")?;
        social::check(
            self.boundary.seconds().to_bits() == now.seconds().to_bits(),
            "social boundary disagreement",
        )?;
        validate_warm(&self.state)
    }
    pub fn cost(&self) -> Result<SocialCost> {
        Ok(aggregate::measure(self, "social")?.into())
    }
}
impl Admitted<WarmExchangesDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, "social", r))
    }
    pub fn into_candidate(self, now: LogicalTime) -> Result<Admitted<WarmExchangesCandidate>> {
        self.try_map(|d, _| {
            d.validate(now)?;
            Ok(WarmExchangesCandidate { data: d })
        })
    }
}
impl WarmExchangesCandidate {
    pub fn warm_exchanges(&self) -> &WarmExchanges {
        &self.data.state
    }
}

#[derive(Debug, Serialize)]
pub struct NoveltyDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "NoveltyV1")]
    state: Novelty,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoveltyWire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "NoveltyV1")]
    state: Novelty,
}
#[derive(Serialize)]
struct NoveltyView<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "NoveltyV1")]
    state: &'a Novelty,
}
#[derive(Debug)]
pub struct NoveltyCandidate {
    data: NoveltyDtoV1,
}
impl Novelty {
    pub fn export_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<NoveltyDtoV1>> {
        social::prepare(
            &NoveltyView {
                version: 1,
                boundary: now,
                state: self,
            },
            &mut r,
        )?;
        crate::checkpoint::logical("social", now.seconds())?;
        validate_novelty(self)?;
        Ok(Admitted::new(
            NoveltyDtoV1 {
                version: 1,
                boundary: now,
                state: self.clone(),
            },
            r,
        ))
    }
}
impl NoveltyDtoV1 {
    pub fn decode(bytes: &[u8], mut r: Reservation, now: LogicalTime) -> Result<Admitted<Self>> {
        let w: NoveltyWire = aggregate::decode_with_working(
            bytes,
            "social",
            &mut r,
            social::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            state: w.state,
        };
        d.validate(now)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, now: LogicalTime) -> Result<()> {
        crate::checkpoint::logical("social", now.seconds())?;
        social::check(self.version == 1, "unsupported social version")?;
        social::check(
            self.boundary.seconds().to_bits() == now.seconds().to_bits(),
            "social boundary disagreement",
        )?;
        validate_novelty(&self.state)
    }
    pub fn cost(&self) -> Result<SocialCost> {
        Ok(aggregate::measure(self, "social")?.into())
    }
}
impl Admitted<NoveltyDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, "social", r))
    }
    pub fn into_candidate(self, now: LogicalTime) -> Result<Admitted<NoveltyCandidate>> {
        self.try_map(|d, _| {
            d.validate(now)?;
            Ok(NoveltyCandidate { data: d })
        })
    }
}
impl NoveltyCandidate {
    pub fn novelty(&self) -> &Novelty {
        &self.data.state
    }
}

#[cfg(test)]
mod tests;
