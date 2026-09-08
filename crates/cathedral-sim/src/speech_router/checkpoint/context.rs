use super::*;
use crate::{
    receipts::{CheckpointReceiptRef, CommandLedger, CommandLedgerDtoV1},
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
use std::collections::BTreeSet;
#[derive(Clone, Copy)]
enum Ledger<'a> {
    Live(&'a CommandLedger),
    Saved(&'a CommandLedgerDtoV1),
}
/// Context borrows the exact receipt owner. World.speech_actions is reconstructed
/// from accepted rows for an unadopted backbone, whose inventory declares it R.
#[derive(Clone, Copy)]
pub struct SpeechCheckpointContext<'a> {
    pub(crate) now: LogicalTime,
    pub(crate) backbone: BackboneRefs<'a>,
    ledger: Ledger<'a>,
    speech_actions: Option<&'a BTreeSet<CommandId>>,
}
impl<'a> SpeechCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            ledger: Ledger::Live(&w.command_ledger),
            speech_actions: Some(&w.speech_actions),
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
            speech_actions: None,
        }
    }
    pub(crate) fn validate(self) -> Result<()> {
        crate::checkpoint::logical(OWNER, self.now.seconds())?;
        check(
            self.backbone.characters.len() <= 25_000,
            "speech actor context count",
        )?;
        // Called only after prepare/decode reserves VALIDATION_WORKING_BYTES.
        // Saved ledger validate uses bounded BTreeSet scratch, drops it here,
        // and never constructs a candidate ledger/World (M2a13 ADMISSION).
        match self.ledger {
            Ledger::Live(l) => l.validate_checkpoint_boundary(),
            Ledger::Saved(l) => l.validate(self.now),
        }
    }
    pub(super) fn receipt(self, id: CommandId) -> Option<CheckpointReceiptRef<'a>> {
        match self.ledger {
            Ledger::Live(l) => l.checkpoint_speech_receipt(id),
            Ledger::Saved(l) => l.checkpoint_speech_receipt(id),
        }
    }
    pub(super) fn owners(self, ids: impl Iterator<Item = CommandId> + Clone) -> Result<()> {
        if let Some(s) = self.speech_actions {
            check(
                s.len() == ids.clone().count() && ids.into_iter().all(|id| s.contains(&id)),
                "speech_actions owner disagreement",
            )?;
        }
        Ok(())
    }
}
