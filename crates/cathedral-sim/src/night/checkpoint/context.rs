use super::*;
use crate::{
    clock::WorldClockDtoV1,
    receipts::{CommandLedger, CommandLedgerDtoV1},
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
#[derive(Clone, Copy)]
enum Ledger<'a> {
    Live(&'a CommandLedger),
    Saved(&'a CommandLedgerDtoV1),
}
#[derive(Clone, Copy)]
pub struct NightCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    clock: &'a WorldClock,
    ledger: Ledger<'a>,
}
impl<'a> NightCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime, clock: &'a WorldClock) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            clock,
            ledger: Ledger::Live(&w.command_ledger),
        }
    }
    pub fn from_backbone(
        b: &'a BackboneCandidate,
        now: LogicalTime,
        clock: &'a WorldClock,
        ledger: &'a CommandLedgerDtoV1,
    ) -> Self {
        Self {
            now,
            backbone: b.references(),
            clock,
            ledger: Ledger::Saved(ledger),
        }
    }
    pub(crate) fn root(self, id: OperationId) -> bool {
        match self.ledger {
            Ledger::Live(l) => l.checkpoint_night_root(id),
            Ledger::Saved(l) => l.checkpoint_night_root(id),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingV1 {
    clock: WorldClockDtoV1,
}
impl BindingV1 {
    pub(crate) fn new(c: NightCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.backbone.characters.len() <= MAX_PERSONS,
            "Night actor context count",
        )?;
        match c.ledger {
            Ledger::Live(l) => l.validate_checkpoint_boundary()?,
            Ledger::Saved(l) => l.validate(c.now)?,
        }
        Ok(Self {
            clock: c.clock.checkpoint_v1(c.now)?,
        })
    }
}

impl PartialEq for BindingV1 {
    fn eq(&self, other: &Self) -> bool {
        self.clock.checkpoint_bits_eq(&other.clock)
    }
}
