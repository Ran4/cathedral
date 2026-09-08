//! Explicit records: JSON object keys cannot represent structured CommandIds.
use super::*;
use crate::checkpoint::{
    self, Admitted, BoundedText, BoundedVec, CheckpointError, Cohort, Reservation, Result,
    encoded_len,
};
use crate::timeline::LogicalTime;

const OWNER: &str = "command_ledger";
pub const MAX_ENCODED_BYTES: usize = 5 * 1024 * 1024;
/// Candidate DTO + reconstructed BTree indexes + validation scratch. Encoding
/// or input bytes are charged separately. This is a conservative peak allowance,
/// not a claim about measured process RSS or the final complete checkpoint.
pub const WORKING_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECENT_HEAP_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperationIdV1 {
    producer: u8,
    sequence: u64,
}
impl From<OperationId> for OperationIdV1 {
    fn from(id: OperationId) -> Self {
        Self {
            producer: id.producer,
            sequence: id.sequence,
        }
    }
}
impl From<OperationIdV1> for OperationId {
    fn from(id: OperationIdV1) -> Self {
        Self {
            producer: id.producer,
            sequence: id.sequence,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandIdV1 {
    operation: OperationIdV1,
    step: u16,
}
impl From<CommandId> for CommandIdV1 {
    fn from(id: CommandId) -> Self {
        Self {
            operation: id.operation.into(),
            step: id.step,
        }
    }
}
impl From<CommandIdV1> for CommandId {
    fn from(id: CommandIdV1) -> Self {
        Self {
            operation: id.operation.into(),
            step: id.step,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerV1 {
    issued: u64,
    high_water: u64,
    compacted_floor: u64,
}
impl From<ProducerState> for ProducerV1 {
    fn from(p: ProducerState) -> Self {
        Self {
            issued: p.issued,
            high_water: p.high_water,
            compacted_floor: p.compacted_floor,
        }
    }
}
impl From<ProducerV1> for ProducerState {
    fn from(p: ProducerV1) -> Self {
        Self {
            issued: p.issued,
            high_water: p.high_water,
            compacted_floor: p.compacted_floor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefV1 {
    kind: BoundedText<16>,
    id: BoundedText<64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeV1 {
    state: ReceiptState,
    code: BoundedText<48>,
    message: BoundedText<192>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptV1 {
    id: CommandIdV1,
    ordinal: u64,
    at: LogicalTime,
    outcome: OutcomeV1,
    affected: BoundedVec<RefV1, 2>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryV1 {
    payload_version: u16,
    digest: [u8; 32],
    receipt: ReceiptV1,
}

fn err(reason: impl Into<String>) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn text<const N: usize>(s: &str) -> Result<BoundedText<N>> {
    BoundedText::new(s).map_err(err)
}
fn name(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(char::is_control)
}
impl EntryV1 {
    fn from_entry(e: &LedgerEntry) -> Result<Self> {
        checkpoint::logical(OWNER, e.receipt.at)?;
        if e.receipt.affected.len() > 2 {
            return Err(err("too many principal references"));
        }
        Ok(Self {
            payload_version: e.payload_version,
            digest: e.digest,
            receipt: ReceiptV1 {
                id: e.receipt.id.into(),
                ordinal: e.receipt.ordinal,
                at: LogicalTime::new(e.receipt.at).expect("validated time"),
                outcome: OutcomeV1 {
                    state: e.receipt.outcome.state,
                    code: text(&e.receipt.outcome.code)?,
                    message: text(&e.receipt.outcome.message)?,
                },
                affected: BoundedVec(
                    e.receipt
                        .affected
                        .iter()
                        .map(|r| {
                            Ok(RefV1 {
                                kind: text(&r.kind)?,
                                id: text(&r.id)?,
                            })
                        })
                        .collect::<Result<_>>()?,
                ),
            },
        })
    }
    fn entry(&self) -> LedgerEntry {
        LedgerEntry {
            payload_version: self.payload_version,
            digest: self.digest,
            receipt: Receipt {
                id: self.receipt.id.into(),
                ordinal: self.receipt.ordinal,
                at: self.receipt.at.seconds(),
                outcome: Outcome {
                    state: self.receipt.outcome.state,
                    code: self.receipt.outcome.code.0.clone(),
                    message: self.receipt.outcome.message.0.clone(),
                },
                affected: self
                    .receipt
                    .affected
                    .0
                    .iter()
                    .map(|r| AffectedRef {
                        kind: r.kind.0.clone(),
                        id: r.id.0.clone(),
                    })
                    .collect(),
            },
        }
    }
    fn validate(&self, now: LogicalTime) -> Result<()> {
        checkpoint::logical(OWNER, self.receipt.at.seconds())?;
        if self.payload_version != PAYLOAD_VERSION
            || self.receipt.at > now
            || !name(&self.receipt.outcome.code.0)
            || self.receipt.outcome.code.0 == "dispatch_pending"
            || self.receipt.outcome.message.0.chars().any(char::is_control)
        {
            return Err(err("invalid version, outcome or receipt time"));
        }
        for r in &self.receipt.affected.0 {
            // A receipt records the principal of a past/attempted command. An
            // eaten item, scrubbed mark or rejected absent target is legitimate.
            // Current ownership references are validated by each active owner.
            if !name(&r.id.0)
                || !matches!(
                    r.kind.0.as_str(),
                    "actor" | "item" | "fixture" | "mark" | "ward"
                )
            {
                return Err(err("invalid principal reference kind or identity"));
            }
            if r.kind.0 == "mark" && r.id.0.parse::<u64>().is_err() {
                return Err(err("invalid mark reference identity"));
            }
            if r.kind.0 == "ward"
                && !crate::lore::PlanningWard::ALL
                    .iter()
                    .any(|w| w.as_str() == r.id.0)
            {
                return Err(err("invalid ward reference identity"));
            }
        }
        encoded_len(self, MAX_ENTRY_BYTES).map_err(err)?;
        Ok(())
    }
    fn heap_strings(&self) -> usize {
        self.receipt.outcome.code.0.capacity()
            + self.receipt.outcome.message.0.capacity()
            + self.receipt.affected.0.capacity() * std::mem::size_of::<AffectedRef>()
            + self
                .receipt
                .affected
                .0
                .iter()
                .map(|r| r.kind.0.capacity() + r.id.0.capacity())
                .sum::<usize>()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandLedgerDtoV1 {
    version: u16,
    producers: [ProducerV1; PRODUCER_CAPACITY],
    next_ordinal: u64,
    recent: BoundedVec<EntryV1, RECENT_CAPACITY>,
    retained: BoundedVec<EntryV1, PROTECTED_CAPACITY>,
    protected: BoundedVec<OperationIdV1, PROTECTED_CAPACITY>,
}
impl CommandLedger {
    pub(crate) fn validate_checkpoint_boundary(&self) -> Result<()> {
        if !self.pending.is_empty() || !self.updates.is_empty() {
            return Err(err("incomplete dispatch or receipt notification flush"));
        }
        Ok(())
    }

    /// Must run after the ordinary receipt-notification flush. The eventual
    /// envelope preserves already emitted, unread host receipts separately.
    pub fn checkpoint_v1(
        &self,
        now: LogicalTime,
        reservation: Reservation,
    ) -> Result<Admitted<CommandLedgerDtoV1>> {
        reservation.require(Cohort::SavePayload, WORKING_BYTES)?;
        self.validate_checkpoint_boundary()?;
        if self.recent.len() > RECENT_CAPACITY
            || self.retained.len() > PROTECTED_CAPACITY
            || self.protected.len() > PROTECTED_CAPACITY
        {
            return Err(err("ledger count limit exceeded"));
        }
        // Check map key/value agreement; array export must not hide a malformed
        // runtime index by silently discarding its keys.
        if self
            .recent
            .iter()
            .chain(&self.retained)
            .any(|(id, e)| *id != e.receipt.id)
        {
            return Err(err("receipt map key mismatch"));
        }
        let dto = CommandLedgerDtoV1 {
            version: 1,
            producers: self.producers.map(Into::into),
            next_ordinal: self.next_ordinal,
            recent: BoundedVec(
                self.recent
                    .values()
                    .map(EntryV1::from_entry)
                    .collect::<Result<_>>()?,
            ),
            retained: BoundedVec(
                self.retained
                    .values()
                    .map(EntryV1::from_entry)
                    .collect::<Result<_>>()?,
            ),
            protected: BoundedVec(self.protected.iter().copied().map(Into::into).collect()),
        };
        dto.validate(now)?;
        Ok(Admitted::new(dto, reservation))
    }
}
impl CommandLedgerDtoV1 {
    pub const MAX_ENCODED_BYTES: usize = MAX_ENCODED_BYTES;
    pub const WORKING_BYTES: usize = WORKING_BYTES;
    pub fn decode_json(
        bytes: &[u8],
        now: LogicalTime,
        reservation: Reservation,
    ) -> Result<Admitted<Self>> {
        let dto: Self =
            checkpoint::decode(bytes, OWNER, MAX_ENCODED_BYTES, &reservation, WORKING_BYTES)?;
        dto.validate(now)?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn encode_json(dto: Admitted<Self>, now: LogicalTime) -> Result<Admitted<Vec<u8>>> {
        dto.try_map(|value, reservation| {
            value.validate(now)?;
            checkpoint::encode(&value, OWNER, MAX_ENCODED_BYTES, reservation, WORKING_BYTES)
        })
    }
    /// Local structural/retention validation. This does not claim whole-world
    /// ownership agreement; the eventual envelope must also validate_owner_roots.
    pub fn validate(&self, now: LogicalTime) -> Result<()> {
        checkpoint::logical(OWNER, now.seconds())?;
        if self.version != 1
            || self.recent.0.len() > RECENT_CAPACITY
            || self.retained.0.len() > PROTECTED_CAPACITY
            || self.protected.0.len() > PROTECTED_CAPACITY
        {
            return Err(err("unsupported version or ledger count"));
        }
        for p in &self.producers {
            // Issued and received high-water are independent. A fully exhausted
            // u64 counter is valid saved state: the ordinary allocator refuses.
            if p.compacted_floor > p.high_water {
                return Err(err("compacted floor exceeds accepted high-water"));
            }
        }
        let valid_root = |id: OperationId| {
            self.producers
                .get(usize::from(id.producer))
                .is_some_and(|p| id.sequence > 0 && id.sequence <= p.issued.max(p.high_water))
        };
        let mut roots = BTreeSet::new();
        for root in &self.protected.0 {
            let id: OperationId = (*root).into();
            if !valid_root(id) || !roots.insert(id) {
                return Err(err("invalid or duplicate protected root"));
            }
        }
        let mut ids = BTreeSet::new();
        let mut ordinals = BTreeSet::new();
        for (retained, rows) in [(false, &self.recent.0), (true, &self.retained.0)] {
            for e in rows {
                e.validate(now)?;
                let id: CommandId = e.receipt.id.into();
                if !valid_root(id.operation)
                    || id.step > MAX_STEPS
                    || id.operation.sequence
                        > self.producers[usize::from(id.operation.producer)].high_water
                    || e.receipt.ordinal == 0
                    || e.receipt.ordinal > self.next_ordinal
                    || !ids.insert(id)
                    || !ordinals.insert(e.receipt.ordinal)
                    || (retained && !roots.contains(&id.operation))
                    || (matches!(
                        e.receipt.outcome.state,
                        ReceiptState::Accepted | ReceiptState::InProgress
                    ) && !roots.contains(&id.operation))
                {
                    return Err(err(
                        "invalid/duplicate receipt identity, ordinal or protected ownership",
                    ));
                }
            }
        }
        if self.recent_heap_upper_bound() > MAX_RECENT_HEAP_BYTES {
            return Err(err("recent ledger allocation limit exceeded"));
        }
        encoded_len(self, MAX_ENCODED_BYTES).map_err(err)?;
        Ok(())
    }
    pub fn validate_owner_roots(&self, owners: &BTreeSet<OperationId>) -> Result<()> {
        let saved: BTreeSet<OperationId> =
            self.protected.0.iter().copied().map(Into::into).collect();
        if &saved != owners {
            Err(err(
                "protected roots disagree with complete semantic owner inventory",
            ))
        } else {
            Ok(())
        }
    }
    /// Exercise the deterministic candidate index construction without exposing
    /// an adoptable ledger before the complete owner inventory is implemented.
    pub fn validate_index_build(&self, now: LogicalTime) -> Result<()> {
        self.candidate(now).map(|_| ())
    }
    /// Conservative reconstructed recent-tree allocation, independent of encoded
    /// size. Rust's BTree nodes have at most 11 pairs and every non-root node at
    /// least 5; allow a full node plus 64 bytes header and 12 child pointers.
    pub fn recent_heap_upper_bound(&self) -> usize {
        let n = self.recent.0.len();
        let nodes = if n == 0 { 0 } else { (n - 1) / 5 + 1 };
        nodes
            * (64
                + 12 * std::mem::size_of::<usize>()
                + 11 * (std::mem::size_of::<CommandId>() + std::mem::size_of::<LedgerEntry>()))
            + self
                .recent
                .0
                .iter()
                .map(EntryV1::heap_strings)
                .sum::<usize>()
    }
    /// Candidate construction only, kept crate-private until the full envelope
    /// can prove every semantic owner. This never calls command admission.
    pub(crate) fn candidate(&self, now: LogicalTime) -> Result<CommandLedger> {
        self.validate(now)?;
        Ok(CommandLedger {
            producers: self.producers.map(Into::into),
            next_ordinal: self.next_ordinal,
            recent: self
                .recent
                .0
                .iter()
                .map(|e| (e.receipt.id.into(), e.entry()))
                .collect(),
            retained: self
                .retained
                .0
                .iter()
                .map(|e| (e.receipt.id.into(), e.entry()))
                .collect(),
            protected: self.protected.0.iter().copied().map(Into::into).collect(),
            pending: BTreeSet::new(),
            updates: BTreeSet::new(),
        })
    }
}
