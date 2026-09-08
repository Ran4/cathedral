use super::*;
use crate::{
    receipts::{CommandLedger, CommandLedgerDtoV1},
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
#[derive(Clone, Copy)]
enum Ledger<'a> {
    Live(&'a CommandLedger),
    Saved(&'a CommandLedgerDtoV1),
}
/// Borrowed authoritative boundary. Historical scheduler subjects need not
/// still be present; this context validates the obligation ledger, not a roster
/// reconstruction. Full protected-root owner equality belongs to the envelope.
#[derive(Clone, Copy)]
pub struct SchedulerCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    ledger: Ledger<'a>,
}
impl<'a> SchedulerCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            ledger: Ledger::Live(&w.command_ledger),
        }
    }
    pub fn from_backbone(
        b: &'a BackboneCandidate,
        now: LogicalTime,
        ledger: &'a CommandLedgerDtoV1,
    ) -> Self {
        Self {
            now,
            backbone: b.references(),
            ledger: Ledger::Saved(ledger),
        }
    }
    pub(crate) fn validate(self) -> Result<()> {
        checkpoint::logical(OWNER, self.now.seconds())?;
        check(
            self.backbone.characters.len() <= MAX_LANE_ACTORS,
            "scheduler actor context count",
        )?;
        match self.ledger {
            Ledger::Live(l) => l.validate_checkpoint_boundary(),
            Ledger::Saved(l) => l.validate(self.now),
        }
    }
    pub(crate) fn root(self, id: OperationId) -> bool {
        match self.ledger {
            Ledger::Live(l) => l.checkpoint_turn_root(id),
            Ledger::Saved(l) => l.checkpoint_turn_root(id),
        }
    }
}
