use super::*;

/// Borrowed receipt projection for the interrupted-speech owner. This never
/// builds a ledger/index. Exact payload digest binding remains the full envelope.
#[allow(private_interfaces)]
pub(crate) enum CheckpointReceiptRef<'a> {
    Live(&'a Receipt),
    Saved(&'a EntryV1),
}
impl Serialize for CheckpointReceiptRef<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Live(r) => r.serialize(s),
            Self::Saved(e) => e.receipt.serialize(s),
        }
    }
}
impl CheckpointReceiptRef<'_> {
    pub(crate) fn validate(&self, now: LogicalTime) -> Result<()> {
        match self {
            Self::Live(r) => validate_speech_receipt(r, now),
            Self::Saved(e) => e.validate(now),
        }
    }
    pub(crate) fn copy(&self) -> Receipt {
        match self {
            Self::Live(r) => (*r).clone(),
            Self::Saved(e) => e.entry().receipt,
        }
    }
    pub(crate) fn matches(&self, r: &Receipt) -> bool {
        match self {
            Self::Live(saved) => {
                saved.id == r.id
                    && saved.ordinal == r.ordinal
                    && saved.at.to_bits() == r.at.to_bits()
                    && saved.outcome == r.outcome
                    && saved.affected == r.affected
            }
            Self::Saved(e) => {
                let s = &e.receipt;
                CommandId::from(s.id) == r.id
                    && s.ordinal == r.ordinal
                    && s.at.seconds().to_bits() == r.at.to_bits()
                    && s.outcome.state == r.outcome.state
                    && s.outcome.code.0 == r.outcome.code
                    && s.outcome.message.0 == r.outcome.message
                    && s.affected.0.len() == r.affected.len()
                    && s.affected
                        .0
                        .iter()
                        .zip(&r.affected)
                        .all(|(a, b)| a.kind.0 == b.kind && a.id.0 == b.id)
            }
        }
    }
}
impl CommandLedger {
    pub(crate) fn checkpoint_speech_receipt(
        &self,
        id: CommandId,
    ) -> Option<CheckpointReceiptRef<'_>> {
        (id.operation.sequence > 0
            && id.step <= MAX_STEPS
            && self.is_protected(id.operation)
            && self
                .producers
                .get(usize::from(id.operation.producer))
                .is_some_and(|p| id.operation.sequence <= p.issued.max(p.high_water)))
        .then(|| self.get(id))
        .flatten()
        .map(CheckpointReceiptRef::Live)
    }
}
impl CommandLedgerDtoV1 {
    pub(crate) fn checkpoint_speech_receipt(
        &self,
        id: CommandId,
    ) -> Option<CheckpointReceiptRef<'_>> {
        if id.operation.sequence == 0
            || id.step > MAX_STEPS
            || !self
                .producers
                .get(usize::from(id.operation.producer))
                .is_some_and(|p| id.operation.sequence <= p.issued.max(p.high_water))
            || !self
                .protected
                .0
                .iter()
                .any(|p| OperationId::from(*p) == id.operation)
        {
            return None;
        }
        self.recent
            .0
            .iter()
            .chain(&self.retained.0)
            .find(|e| CommandId::from(e.receipt.id) == id)
            .map(CheckpointReceiptRef::Saved)
    }
}

/// Same closed Receipt field domain as EntryV1::validate, without conversion,
/// string allocation or reconstruction. Context owns payload/entry validation.
pub(crate) fn validate_speech_receipt(r: &Receipt, now: LogicalTime) -> Result<()> {
    checkpoint::logical(OWNER, r.at)?;
    if r.at > now.seconds()
        || r.ordinal == 0
        || r.outcome.code.len() > 48
        || !name(&r.outcome.code)
        || r.outcome.code == "dispatch_pending"
        || r.outcome.message.len() > 192
        || r.outcome.message.chars().any(char::is_control)
        || r.affected.len() > 2
    {
        return Err(err("invalid speech receipt fields"));
    }
    for a in &r.affected {
        if a.kind.len() > 16
            || a.id.len() > 64
            || !name(&a.id)
            || !matches!(
                a.kind.as_str(),
                "actor" | "item" | "fixture" | "mark" | "ward"
            )
            || (a.kind == "mark" && a.id.parse::<u64>().is_err())
            || (a.kind == "ward"
                && !crate::lore::PlanningWard::ALL
                    .iter()
                    .any(|w| w.as_str() == a.id))
        {
            return Err(err("invalid speech receipt principal reference"));
        }
    }
    Ok(())
}
