//! Ordered, bounded replay admission for consequential requests.
//!
//! Producer sequences identify an intention, never a transport execution or a
//! UI request string. A provider obligation uses step zero for the whole reply
//! and positive steps for its actions. Its identity survives request retries.
//! The engine admits all producers into one ordinal stream. Compaction closes
//! holes by advancing that producer's floor, including IDs never received.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

mod travel;
pub(crate) use travel::{
    bind_round_edit, commit_actor_action, end_round_edit, end_travel, progress_travel,
    reconcile_travel, release_finished_root,
};

pub const RECENT_CAPACITY: usize = 4096;
pub const PROTECTED_CAPACITY: usize = 256;
pub const PRODUCER_CAPACITY: usize = 32;
pub const MAX_ENTRY_BYTES: usize = 1024;
pub const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
pub const MAX_STEPS: u16 = 256;
pub const PAYLOAD_VERSION: u16 = 1;
pub const PROVIDER_REPLY_VERSION: u16 = 1;
pub const LEGACY_PRODUCER: u8 = 0;
pub const HOST_PRODUCER: u8 = 1;
pub const TURN_PRODUCER: u8 = 2;
pub const NIGHT_PRODUCER: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationId {
    pub producer: u8,
    pub sequence: u64,
}

impl OperationId {
    pub fn command(self, step: u16) -> CommandId {
        CommandId {
            operation: self,
            step,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CommandId {
    pub operation: OperationId,
    pub step: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Rejected,
    Accepted,
    InProgress,
    Completed,
    Interrupted,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub state: ReceiptState,
    pub code: String,
    pub message: String,
}

impl Outcome {
    pub fn completed(message: impl AsRef<str>) -> Self {
        Self::new(ReceiptState::Completed, "completed", message.as_ref())
    }

    pub fn rejected(code: impl AsRef<str>, message: impl AsRef<str>) -> Self {
        Self::new(ReceiptState::Rejected, code.as_ref(), message.as_ref())
    }

    pub fn new(state: ReceiptState, code: &str, message: &str) -> Self {
        Self {
            state,
            code: if code.len() <= 48 && !code.chars().any(char::is_control) {
                code.to_owned()
            } else {
                "invalid_reason_code".to_owned()
            },
            message: bounded_text(message, 192),
        }
    }

    pub fn succeeded(&self) -> bool {
        matches!(
            self.state,
            ReceiptState::Accepted | ReceiptState::InProgress | ReceiptState::Completed
        )
    }
}

/// At most two principal references, not an exhaustive witness/recipient list.
/// Oversized optional detail is omitted; stable IDs are never truncated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedRef {
    pub kind: String,
    pub id: String,
}

impl AffectedRef {
    pub fn new(kind: &str, id: &str) -> Option<Self> {
        if kind.len() > 16
            || id.len() > 64
            || kind.is_empty()
            || id.is_empty()
            || kind.chars().any(char::is_control)
            || id.chars().any(char::is_control)
        {
            return None;
        }
        Some(Self {
            kind: kind.to_owned(),
            id: id.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub id: CommandId,
    pub ordinal: u64,
    /// Accepted logical elapsed seconds, validated at the engine boundary.
    pub at: f64,
    pub outcome: Outcome,
    pub affected: Vec<AffectedRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub payload_version: u16,
    pub digest: [u8; 32],
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerState {
    pub issued: u64,
    pub high_water: u64,
    pub compacted_floor: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommandLedger {
    pub(crate) producers: [ProducerState; PRODUCER_CAPACITY],
    pub(crate) next_ordinal: u64,
    /// Indexed by stable ID; ordinal determines eviction across producers.
    pub(crate) recent: BTreeMap<CommandId, LedgerEntry>,
    /// Protected semantic roots are never discarded to make room for traffic.
    pub(crate) protected: BTreeSet<OperationId>,
    pub(crate) retained: BTreeMap<CommandId, LedgerEntry>,
    /// Dispatch-local reservations. Empty at every completed Engine poll;
    /// checkpoint capture never cuts through a transaction.
    pending: BTreeSet<CommandId>,
    /// Coalesced presentation updates refer to retained records, never duplicate
    /// their strings. Drained at the ordinary flush; not checkpoint authority.
    updates: BTreeSet<CommandId>,
}

#[derive(Debug, PartialEq)]
pub enum Admission {
    New(Ticket),
    Replay(Receipt),
    /// Retryable transport admission. No domain receipt has been committed.
    Deferred(Outcome),
    Refused(Outcome),
}

#[derive(Debug, PartialEq)]
pub struct Ticket {
    pub(crate) id: CommandId,
    ordinal: u64,
    digest: [u8; 32],
}

/// A reply is reserved as one bounded transaction before its first side effect.
pub enum BatchAdmission {
    New(Vec<Ticket>),
    Replay(Receipt),
    Deferred(Outcome),
    Refused(Outcome),
}

impl CommandLedger {
    pub fn reserve_operation(&mut self, producer: u8) -> Result<OperationId, Outcome> {
        if self.protected.len() >= PROTECTED_CAPACITY {
            return Err(Outcome::rejected(
                "active_capacity",
                "active operation capacity is full",
            ));
        }
        let id = self.issue(producer)?;
        self.protect(id)?;
        Ok(id)
    }

    /// The first row is the whole reply (step zero); following rows are its
    /// parsed actions. All belong to the already protected semantic obligation.
    /// A replay of the root returns before reserving or applying anything else.
    pub fn begin_batch(&mut self, rows: &[(CommandId, Value)]) -> BatchAdmission {
        let Some((root, payload)) = rows.first() else {
            return BatchAdmission::Refused(invalid_producer());
        };
        if rows.len() > usize::from(MAX_STEPS) + 1 || root.step != 0 {
            return BatchAdmission::Refused(Outcome::rejected(
                "reply_action_limit",
                "reply exceeds its bounded action count",
            ));
        }
        let root_digest = match payload_digest(payload) {
            Ok(d) => d,
            Err(e) => return BatchAdmission::Refused(e),
        };
        if let Some(entry) = self.entry(*root) {
            if self.pending.contains(root) {
                return BatchAdmission::Deferred(Outcome::rejected(
                    "dispatch_pending",
                    "reply is already being applied",
                ));
            }
            return if entry.digest == root_digest {
                BatchAdmission::Replay(entry.receipt.clone())
            } else {
                BatchAdmission::Refused(Outcome::rejected(
                    "payload_conflict",
                    "this semantic reply was already committed with different content",
                ))
            };
        }
        if !self.protected.contains(&root.operation) {
            return BatchAdmission::Refused(Outcome::rejected(
                "unknown_obligation",
                "reply has no protected semantic obligation",
            ));
        }
        let mut identities = BTreeSet::new();
        for (id, value) in rows {
            if id.operation != root.operation
                || id.step > MAX_STEPS
                || !identities.insert(*id)
                || self.entry(*id).is_some()
            {
                return BatchAdmission::Refused(Outcome::rejected(
                    "invalid_reply_steps",
                    "reply step identities are inconsistent",
                ));
            }
            if let Err(reason) = payload_digest(value) {
                return BatchAdmission::Refused(reason);
            }
        }
        if self.next_ordinal.checked_add(rows.len() as u64).is_none() {
            return BatchAdmission::Refused(Outcome::rejected(
                "identity_exhausted",
                "command order exhausted",
            ));
        }
        let needed = (self.recent.len() + rows.len()).saturating_sub(RECENT_CAPACITY);
        let mut candidates: Vec<_> = self
            .recent
            .iter()
            .filter(|(id, _)| !self.pending.contains(id))
            .collect();
        candidates.sort_unstable_by_key(|(_, entry)| entry.receipt.ordinal);
        if candidates.len() < needed
            || self.retained.len()
                + candidates
                    .iter()
                    .take(needed)
                    .filter(|(id, _)| {
                        self.protected.contains(&id.operation) || self.updates.contains(id)
                    })
                    .count()
                > PROTECTED_CAPACITY
        {
            return BatchAdmission::Deferred(Outcome::rejected(
                "replay_capacity",
                "the complete reply cannot fit the replay window yet",
            ));
        }
        let mut tickets = Vec::with_capacity(rows.len());
        for (id, payload) in rows {
            let Admission::New(ticket) = self.begin(*id, payload) else {
                unreachable!("protected batch admission was preflighted before mutation")
            };
            tickets.push(ticket);
        }
        BatchAdmission::New(tickets)
    }

    /// One-shot adapters call this once per intentional command. Retrying a
    /// transport delivery reuses its already issued identity instead.
    pub fn issue(&mut self, producer: u8) -> Result<OperationId, Outcome> {
        let state = self
            .producers
            .get_mut(usize::from(producer))
            .ok_or_else(invalid_producer)?;
        state.issued = state
            .issued
            .max(state.high_water)
            .checked_add(1)
            .ok_or_else(|| {
                Outcome::rejected("identity_exhausted", "command identities exhausted")
            })?;
        Ok(OperationId {
            producer,
            sequence: state.issued,
        })
    }

    pub(crate) fn command_pending(&self, id: CommandId) -> bool {
        self.pending.contains(&id)
    }

    pub fn is_protected(&self, id: OperationId) -> bool {
        self.protected.contains(&id)
    }

    pub fn protect(&mut self, id: OperationId) -> Result<(), Outcome> {
        if self.protected.contains(&id) {
            return Ok(());
        }
        let producer = self
            .producers
            .get(usize::from(id.producer))
            .ok_or_else(invalid_producer)?;
        if id.sequence == 0 || id.sequence > producer.issued.max(producer.high_water) {
            return Err(invalid_producer());
        }
        if id.sequence <= producer.compacted_floor {
            return Err(Outcome::rejected(
                "command_expired",
                "a forgotten operation cannot be protected again",
            ));
        }
        if self.protected.len() >= PROTECTED_CAPACITY {
            return Err(Outcome::rejected(
                "active_capacity",
                "active operation capacity is full",
            ));
        }
        self.protected.insert(id);
        Ok(())
    }

    pub fn unprotect(&mut self, id: OperationId) {
        self.protected.remove(&id);
        // Forgotten terminal entries remain behind their compacted floor.
        if self.retained.keys().any(|key| key.operation == id) {
            let producer = &mut self.producers[usize::from(id.producer)];
            producer.compacted_floor = producer.compacted_floor.max(id.sequence);
        }
        self.retained
            .retain(|key, _| key.operation != id || self.updates.contains(key));
    }

    pub fn get(&self, id: CommandId) -> Option<&Receipt> {
        self.entry(id).map(|entry| &entry.receipt)
    }

    fn entry(&self, id: CommandId) -> Option<&LedgerEntry> {
        self.recent.get(&id).or_else(|| self.retained.get(&id))
    }

    pub fn begin(&mut self, id: CommandId, payload: &Value) -> Admission {
        if usize::from(id.operation.producer) >= PRODUCER_CAPACITY
            || id.operation.sequence == 0
            || id.step > MAX_STEPS
        {
            return Admission::Refused(invalid_producer());
        }
        let digest = match payload_digest(payload) {
            Ok(digest) => digest,
            Err(reason) => return Admission::Refused(reason),
        };
        // A protected record may predate the compacted floor. Look it up first.
        if let Some(entry) = self.entry(id) {
            if self.pending.contains(&id) {
                return Admission::Deferred(Outcome::rejected(
                    "dispatch_pending",
                    "this command is already being dispatched",
                ));
            }
            return if entry.digest == digest && entry.payload_version == PAYLOAD_VERSION {
                Admission::Replay(entry.receipt.clone())
            } else {
                Admission::Refused(Outcome::rejected(
                    "payload_conflict",
                    "this command identity already names a different request",
                ))
            };
        }
        let producer = &self.producers[usize::from(id.operation.producer)];
        if id.operation.sequence <= producer.compacted_floor
            && !self.protected.contains(&id.operation)
        {
            return Admission::Refused(Outcome::rejected(
                "command_expired",
                "this command is outside the retained replay window",
            ));
        }
        if id.operation.sequence
            > producer
                .high_water
                .max(producer.issued)
                .saturating_add(RECENT_CAPACITY as u64)
        {
            return Admission::Refused(Outcome::rejected(
                "sequence_gap",
                "this producer is too far ahead of its accepted stream",
            ));
        }
        if self.next_ordinal == u64::MAX {
            return Admission::Refused(Outcome::rejected(
                "identity_exhausted",
                "command order exhausted",
            ));
        }
        if self.recent.len() >= RECENT_CAPACITY {
            let Some(oldest) = self
                .recent
                .iter()
                .filter(|(id, _)| !self.pending.contains(id))
                .min_by_key(|(_, e)| e.receipt.ordinal)
                .map(|(id, _)| *id)
            else {
                return Admission::Deferred(Outcome::rejected(
                    "replay_capacity",
                    "pending dispatches fill the replay window",
                ));
            };
            if (self.protected.contains(&oldest.operation) || self.updates.contains(&oldest))
                && self.retained.len() >= PROTECTED_CAPACITY
            {
                return Admission::Deferred(Outcome::rejected(
                    "replay_capacity",
                    "referenced action records fill the replay window",
                ));
            }
            let entry = self.recent.remove(&oldest).expect("oldest exists");
            let old_producer = &mut self.producers[usize::from(oldest.operation.producer)];
            old_producer.compacted_floor =
                old_producer.compacted_floor.max(oldest.operation.sequence);
            if self.protected.contains(&oldest.operation) || self.updates.contains(&oldest) {
                self.retained.insert(oldest, entry);
            }
        }
        self.next_ordinal = match self.next_ordinal.checked_add(1) {
            Some(next) => next,
            None => {
                return Admission::Refused(Outcome::rejected(
                    "identity_exhausted",
                    "command order exhausted",
                ));
            }
        };
        let producer = &mut self.producers[usize::from(id.operation.producer)];
        producer.high_water = producer.high_water.max(id.operation.sequence);
        self.pending.insert(id);
        self.recent.insert(
            id,
            LedgerEntry {
                payload_version: PAYLOAD_VERSION,
                digest,
                receipt: Receipt {
                    id,
                    ordinal: self.next_ordinal,
                    at: 0.0,
                    outcome: Outcome::rejected("dispatch_pending", "not yet dispatched"),
                    affected: Vec::new(),
                },
            },
        );
        Admission::New(Ticket {
            id,
            ordinal: self.next_ordinal,
            digest,
        })
    }

    pub fn finish(
        &mut self,
        ticket: Ticket,
        at: f64,
        outcome: Outcome,
        affected: Vec<AffectedRef>,
    ) -> Receipt {
        assert!(at.is_finite() && at >= 0.0, "accepted logical receipt time");
        let mut entry = LedgerEntry {
            payload_version: PAYLOAD_VERSION,
            digest: ticket.digest,
            receipt: Receipt {
                id: ticket.id,
                ordinal: ticket.ordinal,
                at,
                outcome: Outcome::new(outcome.state, &outcome.code, &outcome.message),
                affected: affected
                    .iter()
                    .filter_map(|r| AffectedRef::new(&r.kind, &r.id))
                    .take(2)
                    .collect(),
            },
        };
        // Bound the actual encoded record, including escaping, not size_of or
        // character count. References and typed state survive message trimming.
        while serde_json::to_vec(&entry).expect("finite receipt").len() > MAX_ENTRY_BYTES {
            assert!(
                entry.receipt.outcome.message.pop().is_some(),
                "fixed receipt metadata fits its budget"
            );
        }
        let receipt = entry.receipt.clone();
        assert!(
            self.pending.remove(&ticket.id),
            "a dispatch ticket is finished exactly once"
        );
        self.recent.insert(ticket.id, entry);
        receipt
    }

    pub fn encoded_recent_bytes(&self) -> usize {
        self.recent
            .values()
            .map(|entry| serde_json::to_vec(entry).expect("finite receipt").len())
            .sum()
    }

    pub fn is_at_boundary(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn recent_len(&self) -> usize {
        self.recent.len()
    }

    pub fn retained_len(&self) -> usize {
        self.retained.len()
    }

    pub(crate) fn operation_pending(&self, id: OperationId) -> bool {
        self.pending.iter().any(|key| key.operation == id)
    }

    /// Advance an existing accepted undertaking. Terminal results are immutable;
    /// duplicate delivery observes the retained result without another update.
    pub fn advance(
        &mut self,
        id: CommandId,
        at: f64,
        outcome: Outcome,
    ) -> Result<Receipt, Outcome> {
        if !at.is_finite() || at < 0.0 || self.pending.contains(&id) {
            return Err(Outcome::rejected(
                "invalid_progress",
                "progress requires a committed receipt and finite accepted time",
            ));
        }
        let Some(entry) = self
            .recent
            .get_mut(&id)
            .or_else(|| self.retained.get_mut(&id))
        else {
            return Err(Outcome::rejected(
                "command_expired",
                "the undertaking receipt is no longer retained",
            ));
        };
        if at < entry.receipt.at
            || (entry.receipt.outcome.state == ReceiptState::InProgress
                && outcome.state == ReceiptState::Accepted)
        {
            return Err(Outcome::rejected(
                "invalid_progress",
                "undertaking time and progress cannot move backwards",
            ));
        }
        if !matches!(
            entry.receipt.outcome.state,
            ReceiptState::Accepted | ReceiptState::InProgress
        ) {
            return Ok(entry.receipt.clone());
        }
        entry.receipt.at = at;
        entry.receipt.outcome = Outcome::new(outcome.state, &outcome.code, &outcome.message);
        while serde_json::to_vec(&entry).expect("finite receipt").len() > MAX_ENTRY_BYTES {
            if entry.receipt.outcome.message.pop().is_none() {
                break;
            }
        }
        self.updates.insert(id);
        Ok(entry.receipt.clone())
    }

    pub fn drain_updates(&mut self) -> Vec<Receipt> {
        let updates = std::mem::take(&mut self.updates);
        let receipts = updates
            .iter()
            .filter_map(|id| self.get(*id).cloned())
            .collect();
        self.retained
            .retain(|id, _| self.protected.contains(&id.operation));
        receipts
    }
}

fn invalid_producer() -> Outcome {
    Outcome::rejected(
        "invalid_identity",
        "command producer, sequence or step is invalid",
    )
}

/// V1: recursively sorted JSON object keys, serde_json scalar/array encoding,
/// UTF-8 bytes, SHA-256 with this exact domain/version prefix. No Debug hashes.
pub fn payload_digest(payload: &Value) -> Result<[u8; 32], Outcome> {
    use serde::ser::{SerializeMap, SerializeSeq};
    // Inspect borrowed input first: no deep clone or unbounded serialized Vec.
    fn preflight(value: &Value, depth: usize, remaining: &mut usize) -> bool {
        if depth > 32 {
            return false;
        }
        let bytes = match value {
            Value::String(s) => s.len().saturating_add(2),
            _ => 2,
        };
        let Some(left) = remaining.checked_sub(bytes) else {
            return false;
        };
        *remaining = left;
        match value {
            Value::Object(map) => map.iter().all(|(key, value)| {
                let Some(left) = remaining.checked_sub(key.len().saturating_add(3)) else {
                    return false;
                };
                *remaining = left;
                preflight(value, depth + 1, remaining)
            }),
            Value::Array(values) => values
                .iter()
                .all(|value| preflight(value, depth + 1, remaining)),
            _ => true,
        }
    }
    let mut remaining = MAX_PAYLOAD_BYTES;
    if !preflight(payload, 0, &mut remaining) {
        return Err(Outcome::rejected(
            "payload_too_large",
            "command exceeds its payload or nesting limit",
        ));
    }
    struct Canonical<'a>(&'a Value);
    impl Serialize for Canonical<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            match self.0 {
                Value::Object(values) => {
                    let mut fields: Vec<_> = values.iter().collect();
                    fields.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
                    let mut map = serializer.serialize_map(Some(fields.len()))?;
                    for (key, value) in fields {
                        map.serialize_entry(key, &Canonical(value))?;
                    }
                    map.end()
                }
                Value::Array(values) => {
                    let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                    for value in values {
                        sequence.serialize_element(&Canonical(value))?;
                    }
                    sequence.end()
                }
                scalar => scalar.serialize(serializer),
            }
        }
    }
    // Write is only serde_json's memory-sink interface; this sink performs no IO.
    struct HashSink {
        hash: Sha256,
        bytes: usize,
    }
    impl std::io::Write for HashSink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.saturating_add(bytes.len()) > MAX_PAYLOAD_BYTES {
                return Err(std::io::Error::other("payload limit"));
            }
            self.bytes += bytes.len();
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"cathedral-command-payload-v1\0");
    let mut sink = HashSink { hash, bytes: 0 };
    serde_json::to_writer(&mut sink, &Canonical(payload)).map_err(|_| {
        Outcome::rejected(
            "payload_too_large",
            "encoded command exceeds its payload limit",
        )
    })?;
    Ok(sink.hash.finalize().into())
}

pub(crate) fn provider_reply_within_limit(reply: &str) -> bool {
    reply.len() <= crate::MAX_LLM_REPLY_CHARS * 4
        && reply.chars().count() <= crate::MAX_LLM_REPLY_CHARS
}

/// Whole provider replies keep the existing Unicode-scalar limit. Hash their
/// exact bounded UTF-8 bytes directly: comments and parse errors also belong to
/// the semantic reply, while each action retains its separate 16-KiB envelope.
pub(crate) fn provider_reply_digest(reply: &str) -> [u8; 32] {
    debug_assert!(reply.len() <= crate::MAX_LLM_REPLY_CHARS * 4);
    let mut hash = Sha256::new();
    hash.update(b"cathedral-provider-reply-v1\0");
    hash.update(reply.as_bytes());
    hash.finalize().into()
}

fn bounded_text(text: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(text.len().min(bytes));
    for character in text.chars().map(|c| if c.is_control() { ' ' } else { c }) {
        if out.len() + character.len_utf8() > bytes {
            break;
        }
        out.push(character);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn commit(ledger: &mut CommandLedger, id: CommandId, payload: &Value) -> Receipt {
        let Admission::New(ticket) = ledger.begin(id, payload) else {
            panic!("new command expected")
        };
        ledger.finish(ticket, 1.0, Outcome::completed("committed"), vec![])
    }

    fn next(ledger: &mut CommandLedger) -> CommandId {
        ledger.issue(HOST_PRODUCER).unwrap().command(0)
    }

    #[test]
    fn compaction_closes_never_received_holes_and_protects_exact_replays() {
        let mut ledger = CommandLedger::default();
        let absent = next(&mut ledger);
        let protected = next(&mut ledger);
        ledger.protect(protected.operation).unwrap();
        let original = commit(&mut ledger, protected, &json!("one"));
        for _ in 0..RECENT_CAPACITY + 3 {
            let id = next(&mut ledger);
            commit(&mut ledger, id, &json!(null));
        }
        assert_eq!(ledger.recent_len(), RECENT_CAPACITY);
        assert_eq!(
            ledger.begin(protected, &json!("one")),
            Admission::Replay(original)
        );
        assert!(
            matches!(ledger.begin(protected, &json!("different")), Admission::Refused(reason) if reason.code == "payload_conflict")
        );
        assert!(
            matches!(ledger.begin(absent, &json!(null)), Admission::Refused(reason) if reason.code == "command_expired")
        );
        // A live undertaking may still spend a new step after recent traffic
        // compacted that producer's older ordinary IDs.
        commit(
            &mut ledger,
            protected.operation.command(1),
            &json!("next step"),
        );
        ledger.unprotect(protected.operation);
        assert!(
            ledger.protect(protected.operation).is_err(),
            "no resurrection"
        );
        assert!(
            matches!(ledger.begin(protected, &json!("one")), Admission::Refused(reason) if reason.code == "command_expired")
        );
    }

    #[test]
    fn outer_provider_ticket_reserves_a_slot_while_nested_steps_fill_the_window() {
        let mut ledger = CommandLedger::default();
        for _ in 0..RECENT_CAPACITY - 1 {
            let id = next(&mut ledger);
            commit(&mut ledger, id, &json!(null));
        }
        let work = ledger.issue(TURN_PRODUCER).unwrap();
        ledger.protect(work).unwrap();
        let Admission::New(outer) = ledger.begin(work.command(0), &json!("whole reply")) else {
            panic!()
        };
        for step in 1..=MAX_STEPS {
            commit(&mut ledger, work.command(step), &json!(step));
        }
        ledger.finish(outer, 1.0, Outcome::completed("reply applied"), vec![]);
        assert!(ledger.is_at_boundary());
        assert_eq!(ledger.recent_len(), RECENT_CAPACITY);
        assert!(ledger.encoded_recent_bytes() <= RECENT_CAPACITY * MAX_ENTRY_BYTES);
    }

    #[test]
    fn protected_saturation_is_retryable_admission_not_a_committed_rejection() {
        let mut ledger = CommandLedger::default();
        let work = ledger.issue(TURN_PRODUCER).unwrap();
        ledger.protect(work).unwrap();
        for step in 0..=MAX_STEPS {
            commit(&mut ledger, work.command(step), &json!(step));
        }
        for _ in 0..RECENT_CAPACITY - 1 {
            let id = next(&mut ledger);
            commit(&mut ledger, id, &json!(null));
        }
        assert_eq!(ledger.retained_len(), PROTECTED_CAPACITY);
        let deferred = next(&mut ledger);
        assert!(
            matches!(ledger.begin(deferred, &json!(null)), Admission::Deferred(reason) if reason.code == "replay_capacity")
        );
        assert!(
            ledger.get(deferred).is_none(),
            "no promised domain rejection"
        );
        ledger.unprotect(work);
        commit(&mut ledger, deferred, &json!(null));
        assert_eq!(ledger.recent_len(), RECENT_CAPACITY);
    }

    #[test]
    fn payload_limits_precede_allocation_and_canonical_digest_is_stable() {
        let left: Value = serde_json::from_str(r#"{"z":[2,1],"a":{"q":true,"b":"x"}}"#).unwrap();
        let right: Value = serde_json::from_str(r#"{"a":{"b":"x","q":true},"z":[2,1]}"#).unwrap();
        assert_eq!(payload_digest(&left), payload_digest(&right));
        assert_ne!(
            payload_digest(&left),
            payload_digest(&json!({"z":[1,2],"a":{"q":true,"b":"x"}}))
        );
        assert!(payload_digest(&json!("x".repeat(MAX_PAYLOAD_BYTES + 1))).is_err());
        assert!(
            payload_digest(&json!("\"".repeat(MAX_PAYLOAD_BYTES / 2))).is_err(),
            "encoded escaping also counts"
        );
        let mut deep = json!(null);
        for _ in 0..34 {
            deep = Value::Array(vec![deep]);
        }
        assert!(payload_digest(&deep).is_err());
        assert!(AffectedRef::new("actor", &"x".repeat(65)).is_none());
    }
    #[test]
    fn escaped_principal_refs_and_max_counters_fit_without_rewriting_identity() {
        assert!(AffectedRef::new("actor", &"\0".repeat(64)).is_none());
        let mut ledger = CommandLedger::default();
        ledger.producers[31].issued = u64::MAX;
        ledger.next_ordinal = u64::MAX - 1;
        let id = OperationId {
            producer: 31,
            sequence: u64::MAX,
        }
        .command(MAX_STEPS);
        let Admission::New(ticket) = ledger.begin(id, &json!({})) else {
            panic!("admitted max counter")
        };
        let reference = AffectedRef::new(&"\\".repeat(16), &"\"".repeat(64)).unwrap();
        let receipt = ledger.finish(
            ticket,
            f64::MAX,
            Outcome::new(
                ReceiptState::Superseded,
                &"\\".repeat(48),
                &"\"".repeat(256),
            ),
            vec![reference.clone(), reference.clone()],
        );
        assert_eq!(receipt.affected, [reference.clone(), reference]);
        assert!(ledger.encoded_recent_bytes() <= MAX_ENTRY_BYTES);
        let old = ledger.clone();
        assert!(matches!(
            ledger.begin(
                OperationId {
                    producer: 1,
                    sequence: 1
                }
                .command(0),
                &json!({})
            ),
            Admission::Refused(_)
        ));
        assert_eq!(ledger, old, "ordinal exhaustion cannot evict a receipt");
    }

    #[test]
    fn undertaking_progress_cannot_predate_acceptance_or_regress_or_reopen_terminal_result() {
        let mut ledger = CommandLedger::default();
        let id = ledger.reserve_operation(TURN_PRODUCER).unwrap().command(1);
        let Admission::New(ticket) = ledger.begin(id, &json!({})) else {
            panic!("admitted")
        };
        assert!(
            ledger
                .advance(id, 1.0, Outcome::completed("premature"))
                .is_err()
        );
        ledger.finish(
            ticket,
            2.0,
            Outcome::new(ReceiptState::Accepted, "accepted", "queued"),
            Vec::new(),
        );
        let original = ledger.clone();
        assert!(
            ledger
                .advance(id, 1.0, Outcome::completed("backward"))
                .is_err()
        );
        assert_eq!(ledger, original);
        ledger
            .advance(
                id,
                3.0,
                Outcome::new(ReceiptState::InProgress, "started", "working"),
            )
            .unwrap();
        let working = ledger.clone();
        assert!(
            ledger
                .advance(
                    id,
                    3.0,
                    Outcome::new(ReceiptState::Accepted, "accepted", "reset")
                )
                .is_err()
        );
        assert_eq!(ledger, working);
        let done = ledger
            .advance(id, 4.0, Outcome::completed("finished"))
            .unwrap();
        assert_eq!(
            ledger
                .advance(
                    id,
                    5.0,
                    Outcome::new(ReceiptState::InProgress, "started", "reopened")
                )
                .unwrap(),
            done
        );
        assert_eq!(ledger.get(id), Some(&done));
    }
}
