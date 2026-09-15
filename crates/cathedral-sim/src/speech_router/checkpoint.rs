//! Semantic input retained for load interruption. This is deliberately not an
//! executable SpeechRouter: no capture/socket/job or automatic speech is restored.
use super::*;
use crate::{
    checkpoint::{Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    receipts::{CommandId, Receipt, ReceiptState},
    scheduler::checkpoint::records::raw_float,
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod context;
pub(crate) mod records;
pub use context::SpeechCheckpointContext;
pub use records::{
    AcceptedRecording, InputPurpose, InterruptedStream, InterruptionStatus, RecordingSource,
};
use records::{RecordingView, StateV1, StreamView};
/// One load's frozen available input plus the truthful new terminal receipts.
/// Original accepted receipt provenance is retained in `state`; neither record
/// is active execution or a protected semantic root after interruption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterruptedSpeech {
    at: LogicalTime,
    state: StateV1,
    #[serde(with = "records::receipt::vec")]
    receipts: Vec<Receipt>,
}
pub const MAX_INTERRUPTED_INPUTS: usize = 64;
pub(crate) const INTERRUPTION_CODE: &str = "recording_interrupted_by_load";
pub(crate) const INTERRUPTION_MESSAGE: &str =
    "Recording interrupted by loading; submit the available text intentionally.";
impl InterruptedSpeech {
    pub fn at(&self) -> LogicalTime {
        self.at
    }
    pub fn captures(&self) -> &[String] {
        &self.state.captures
    }
    pub fn streams(&self) -> &[InterruptedStream] {
        &self.state.streams
    }
    pub fn recordings(&self) -> &[AcceptedRecording] {
        &self.state.accepted_recordings
    }
    pub fn receipts(&self) -> &[Receipt] {
        &self.receipts
    }
    pub fn purpose(&self) -> InputPurpose {
        self.state.purpose
    }
    pub fn status(&self) -> InterruptionStatus {
        self.state.status
    }
    pub(crate) fn count(&self) -> usize {
        self.state.captures.len() + self.state.streams.len() + self.state.accepted_recordings.len()
    }
    pub(crate) fn new(at: LogicalTime, state: StateV1, receipts: Vec<Receipt>) -> Self {
        Self {
            at,
            state,
            receipts,
        }
    }
    pub(crate) fn validate(&self, c: SpeechCheckpointContext<'_>) -> Result<()> {
        check(self.count() > 0, "empty speech interruption group")?;
        check(
            self.at.seconds() <= c.now.seconds(),
            "interruption after capture",
        )?;
        self.state.validate_interrupted(c)?;
        check(
            self.receipts.len() <= MAX_ACTIVE_STREAMS,
            "interruption receipt limit",
        )?;
        for (i, r) in self.receipts.iter().enumerate() {
            crate::receipts::validate_speech_receipt(r, c.now)?;
            check(
                r.outcome.state == ReceiptState::Interrupted
                    && r.at.to_bits() == self.at.seconds().to_bits(),
                "invalid interruption receipt",
            )?;
            check(
                r.outcome.code == INTERRUPTION_CODE
                    && r.outcome.message == INTERRUPTION_MESSAGE
                    && c.history_agrees(r),
                "interruption outcome/history disagreement",
            )?;
            check(
                self.receipts[..i].iter().all(|p| p.id != r.id),
                "duplicate interruption receipt",
            )?;
            check(
                self.state.accepted_recordings.iter().any(|old| {
                    old.semantic == Some(r.id)
                        && old.receipt.as_ref().is_some_and(|prior| {
                            matches!(
                                prior.outcome.state,
                                ReceiptState::Accepted | ReceiptState::InProgress
                            ) && prior.ordinal == r.ordinal
                                && prior.affected == r.affected
                                && prior.at <= r.at
                        })
                }),
                "interruption receipt has no matching accepted provenance",
            )?;
        }
        for t in &self.state.accepted_recordings {
            if let Some(r) = &t.receipt
                && matches!(
                    r.outcome.state,
                    ReceiptState::Accepted | ReceiptState::InProgress
                )
            {
                check(
                    self.receipts.iter().any(|done| done.id == r.id),
                    "accepted input missing interruption outcome",
                )?;
            } else if let Some(r) = &t.receipt {
                check(
                    r.at <= self.at.seconds(),
                    "historical receipt after interruption boundary",
                )?;
                check(c.history_agrees(r), "committed speech history disagreement")?;
            }
        }
        Ok(())
    }
}
pub(crate) fn validate_interrupted(
    groups: &[InterruptedSpeech],
    c: SpeechCheckpointContext<'_>,
) -> Result<()> {
    check(
        groups.len() <= MAX_INTERRUPTED_INPUTS
            && groups.iter().map(InterruptedSpeech::count).sum::<usize>() <= MAX_INTERRUPTED_INPUTS,
        "interrupted input count limit",
    )?;
    for group in groups {
        group.validate(c)?;
    }
    for (i, group) in groups.iter().enumerate() {
        for (j, t) in group.recordings().iter().enumerate() {
            if let Some(id) = t.semantic() {
                check(
                    groups[..i]
                        .iter()
                        .all(|prior| prior.recordings().iter().all(|r| r.semantic() != Some(id))),
                    "duplicate interrupted speech identity",
                )?;
            }
            if let Some(receipt) = &t.receipt {
                // Accepted provenance and its terminal update share one ordinal;
                // distinct command identities never do, even after eviction.
                check(
                    groups[..i]
                        .iter()
                        .flat_map(|g| g.recordings())
                        .chain(&group.recordings()[..j])
                        .all(|prior| {
                            prior
                                .receipt
                                .as_ref()
                                .is_none_or(|p| p.ordinal != receipt.ordinal)
                        }),
                    "duplicate interrupted speech ordinal",
                )?;
            }
        }
    }
    Ok(())
}
pub(crate) fn validate_active_overlap(
    groups: &[InterruptedSpeech],
    active: impl Iterator<Item = CommandId> + Clone,
) -> Result<()> {
    for group in groups {
        for task in group.recordings() {
            if let Some(id) = task.semantic() {
                check(
                    active.clone().all(|live| live != id),
                    "interrupted speech is still active",
                )?;
            }
        }
    }
    Ok(())
}
pub(crate) const OWNER: &str = "speech";
pub const MAX_TEXT_BYTES: usize = 400_000;
pub const MAX_REQUEST_BYTES: usize = 65_536;
/// Saved ledger validation's bounded tree scratch, sequential with constant-space
/// projection scans. No candidate ledger or World is constructed. See ADMISSION.
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpeechCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for SpeechCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn check(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, why))
    }
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<SpeechCost> {
    let c = SpeechCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < c.peak_bytes {
        r.resize(c.peak_bytes)?;
    }
    Ok(c)
}
pub(crate) fn binding(a: LogicalTime, c: SpeechCheckpointContext<'_>) -> Result<()> {
    check(
        a.seconds().to_bits() == c.now.seconds().to_bits(),
        "speech boundary disagreement",
    )
}
fn basename(s: &str) -> Result<()> {
    check(
        s.len() <= 4 * crate::MAX_ID_CHARS && check_basename(s).is_ok(),
        "invalid interrupted basename",
    )
}
fn validate_receipt(r: &Receipt, c: SpeechCheckpointContext<'_>) -> Result<()> {
    crate::receipts::validate_speech_receipt(r, c.now)?;
    check(
        c.receipt(r.id).is_some_and(|saved| saved.matches(r)),
        "speech receipt/root disagreement",
    )
}
fn validate_task(t: &TranscriptionTask, c: SpeechCheckpointContext<'_>) -> Result<()> {
    let TranscriptionTask {
        semantic: _,
        request_id: _,
        basename: _,
        position_m: _,
        backend: _,
        attention: _,
    } = t;
    basename(&t.basename)?;
    check(
        t.request_id.len() <= MAX_REQUEST_BYTES,
        "speech request byte limit",
    )?;
    crate::character::checkpoint::point(t.position_m)?;
    if let Some(id) = t.semantic {
        // The borrowed receipt is serialized/copy-charged as part of the view;
        // validation does not clone it or reconstruct its absent original input.
        let receipt = c
            .receipt(id)
            .ok_or_else(|| CheckpointError::new(OWNER, "speech task receipt/root disagreement"))?;
        receipt.validate(c.now)?;
    }
    Ok(())
}
/// All current fields are explicitly classified here; adding a router field must
/// revisit the owner even if it will be transient. Ignored vectors are not scanned.
pub(crate) struct StateView<'a> {
    router: &'a SpeechRouter,
    context: SpeechCheckpointContext<'a>,
}
impl<'a> StateView<'a> {
    pub(crate) fn new(router: &'a SpeechRouter, context: SpeechCheckpointContext<'a>) -> Self {
        let SpeechRouter {
            interrupted: _,
            stt_stream_grace_seconds: _,
            streams: _,
            captures: _,
            parked: _,
            timings: _,
            recording_jobs: _,
            stream_jobs: _,
            tts_backends: _,
            next_job: _,
            resolved: _,
        } = router;
        Self { router, context }
    }
    fn tasks(
        &self,
    ) -> impl Iterator<Item = (&TranscriptionTask, RecordingSource, Option<f64>)> + Clone {
        self.router
            .recording_jobs
            .iter()
            .map(|(_, t)| (t, RecordingSource::BatchPending, None))
            .chain(
                self.router
                    .parked
                    .iter()
                    .map(|(_, p)| (&p.task, RecordingSource::Parked, Some(p.deadline))),
            )
    }
    fn row(
        &self,
        t: &'a TranscriptionTask,
        source: RecordingSource,
        deadline: Option<f64>,
    ) -> RecordingView<'a> {
        RecordingView {
            source,
            semantic: t.semantic,
            receipt: t.semantic.and_then(|id| self.context.receipt(id)),
            request_id: &t.request_id,
            basename: &t.basename,
            position_m: t.position_m,
            backend: t.backend,
            parked_deadline: deadline,
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let r = self.router;
        self.context.validate()?;
        validate_active_overlap(
            &r.interrupted,
            self.tasks().filter_map(|(t, _, _)| t.semantic),
        )?;
        check(
            r.resolved.is_empty(),
            "synchronous resolved staging is not a completed boundary",
        )?;
        check(
            r.captures.len() <= MAX_ACTIVE_STREAMS
                && r.streams.len() <= MAX_ACTIVE_STREAMS
                && r.recording_jobs.len() + r.parked.len() <= MAX_ACTIVE_STREAMS,
            "speech input count limit",
        )?;
        for (i, (key, _)) in r.captures.iter().enumerate() {
            basename(key)?;
            check(
                r.captures[..i].iter().all(|(p, _)| p != key),
                "duplicate onset key",
            )?;
        }
        for (i, (key, s)) in r.streams.iter().enumerate() {
            let StreamState {
                phase: _,
                next_seq: _,
                decoded_bytes: _,
                end_at: _,
                commit_at: _,
                completed_at: _,
                transcript: _,
                degrade_reason: _,
                status_sent: _,
            } = s;
            basename(key)?;
            check(
                r.streams[..i].iter().all(|(p, _)| p != key),
                "duplicate stream key",
            )?;
            check(
                s.transcript
                    .as_ref()
                    .is_none_or(|t| t.len() <= MAX_TEXT_BYTES),
                "speech available text byte limit",
            )?;
        }
        for (key, p) in &r.parked {
            check(key == &p.task.basename, "parked task basename disagreement")?;
        }
        for (i, (t, _, _)) in self.tasks().enumerate() {
            validate_task(t, self.context)?;
            if let Some(id) = t.semantic {
                check(
                    self.tasks().take(i).all(|(p, _, _)| p.semantic != Some(id)),
                    "duplicate speech command identity",
                )?;
            }
        }
        self.context
            .owners(self.tasks().filter_map(|(t, _, _)| t.semantic))
    }
    pub(crate) fn copy(&self) -> StateV1 {
        StateV1 {
            stt_stream_grace_seconds: self.router.stt_stream_grace_seconds,
            purpose: InputPurpose::PublicPlayerSpeech,
            status: InterruptionStatus::InterruptedUnsent,
            captures: self
                .router
                .captures
                .iter()
                .map(|(s, _)| s.clone())
                .collect(),
            streams: self
                .router
                .streams
                .iter()
                .map(|(s, t)| InterruptedStream {
                    basename: s.clone(),
                    available_text: t.transcript.clone(),
                })
                .collect(),
            accepted_recordings: self
                .tasks()
                .map(|(t, source, deadline)| AcceptedRecording {
                    source,
                    semantic: t.semantic,
                    receipt: t
                        .semantic
                        .and_then(|id| self.context.receipt(id))
                        .map(|r| r.copy()),
                    request_id: t.request_id.clone(),
                    basename: t.basename.clone(),
                    position_m: t.position_m,
                    backend: t.backend,
                    parked_deadline: deadline,
                })
                .collect(),
        }
    }
}
impl Serialize for StateView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::{SerializeSeq, SerializeStruct};
        struct Captures<'a>(&'a SpeechRouter);
        impl Serialize for Captures<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let mut a = s.serialize_seq(Some(self.0.captures.len()))?;
                for (k, _) in &self.0.captures {
                    a.serialize_element(k)?;
                }
                a.end()
            }
        }
        struct Streams<'a>(&'a SpeechRouter);
        impl Serialize for Streams<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let mut a = s.serialize_seq(Some(self.0.streams.len()))?;
                for (k, t) in &self.0.streams {
                    a.serialize_element(&StreamView {
                        basename: k,
                        available_text: &t.transcript,
                    })?;
                }
                a.end()
            }
        }
        struct Accepted<'a, 'b>(&'b StateView<'a>);
        impl Serialize for Accepted<'_, '_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let mut a = s.serialize_seq(Some(
                    self.0.router.recording_jobs.len() + self.0.router.parked.len(),
                ))?;
                for (t, source, deadline) in self.0.tasks() {
                    a.serialize_element(&self.0.row(t, source, deadline))?;
                }
                a.end()
            }
        }
        #[derive(Serialize)]
        struct Raw(#[serde(with = "raw_float")] f64);
        let mut a = s.serialize_struct("StateV1", 6)?;
        a.serialize_field(
            "stt_stream_grace_seconds",
            &Raw(self.router.stt_stream_grace_seconds),
        )?;
        a.serialize_field("purpose", &InputPurpose::PublicPlayerSpeech)?;
        a.serialize_field("status", &InterruptionStatus::InterruptedUnsent)?;
        a.serialize_field("captures", &Captures(self.router))?;
        a.serialize_field("streams", &Streams(self.router))?;
        a.serialize_field("accepted_recordings", &Accepted(self))?;
        a.end()
    }
}
impl StateV1 {
    pub(crate) fn validate(&self, c: SpeechCheckpointContext<'_>) -> Result<()> {
        self.validate_inner(c, true)
    }
    pub(crate) fn validate_interrupted(&self, c: SpeechCheckpointContext<'_>) -> Result<()> {
        self.validate_inner(c, false)
    }
    fn validate_inner(&self, c: SpeechCheckpointContext<'_>, active: bool) -> Result<()> {
        c.validate()?;
        check(
            self.captures.len() <= MAX_ACTIVE_STREAMS
                && self.streams.len() <= MAX_ACTIVE_STREAMS
                && self.accepted_recordings.len() <= MAX_ACTIVE_STREAMS,
            "speech input count limit",
        )?;
        for (i, k) in self.captures.iter().enumerate() {
            basename(k)?;
            check(!self.captures[..i].contains(k), "duplicate onset key")?;
        }
        for (i, t) in self.streams.iter().enumerate() {
            basename(&t.basename)?;
            check(
                self.streams[..i].iter().all(|s| s.basename != t.basename),
                "duplicate stream key",
            )?;
            check(
                t.available_text
                    .as_ref()
                    .is_none_or(|s| s.len() <= MAX_TEXT_BYTES),
                "speech available text byte limit",
            )?;
        }
        let mut parked_seen = false;
        for (i, t) in self.accepted_recordings.iter().enumerate() {
            basename(&t.basename)?;
            check(
                t.request_id.len() <= MAX_REQUEST_BYTES,
                "speech request byte limit",
            )?;
            crate::character::checkpoint::point(t.position_m)?;
            check(
                t.parked_deadline.is_some() == (t.source == RecordingSource::Parked),
                "speech source deadline disagreement",
            )?;
            if t.source == RecordingSource::Parked {
                parked_seen = true;
            } else {
                check(!parked_seen, "speech source order disagreement")?;
            }
            check(
                t.semantic.is_some() == t.receipt.is_some(),
                "speech optional receipt disagreement",
            )?;
            if let Some(id) = t.semantic {
                check(
                    self.accepted_recordings[..i]
                        .iter()
                        .all(|p| p.semantic != Some(id)),
                    "duplicate speech command identity",
                )?;
                let r = t.receipt.as_ref().expect("presence checked");
                check(r.id == id, "speech task receipt identity disagreement")?;
                if active {
                    validate_receipt(r, c)?;
                } else {
                    crate::receipts::validate_speech_receipt(r, c.now)?;
                }
            }
        }
        if active {
            c.owners(self.accepted_recordings.iter().filter_map(|r| r.semantic))
        } else {
            Ok(())
        }
    }
    pub(crate) fn counts(&self, c: SpeechCheckpointContext<'_>) -> SpeechCounts {
        let a = &self.accepted_recordings;
        SpeechCounts {
            characters: c.backbone.characters.len(),
            captures: self.captures.len(),
            streams: self.streams.len(),
            available_texts: self
                .streams
                .iter()
                .filter(|s| s.available_text.is_some())
                .count(),
            available_text_bytes: self
                .streams
                .iter()
                .filter_map(|s| s.available_text.as_ref())
                .map(String::len)
                .sum(),
            accepted_recordings: a.len(),
            batch_pending: a
                .iter()
                .filter(|s| s.source == RecordingSource::BatchPending)
                .count(),
            parked: a
                .iter()
                .filter(|s| s.source == RecordingSource::Parked)
                .count(),
            semantic_receipts: a.iter().filter(|s| s.semantic.is_some()).count(),
            terminal_receipts: a
                .iter()
                .filter_map(|s| s.receipt.as_ref())
                .filter(|r| {
                    !matches!(
                        r.outcome.state,
                        ReceiptState::Accepted | ReceiptState::InProgress
                    )
                })
                .count(),
            unique_roots: a
                .iter()
                .enumerate()
                .filter(|(i, s)| {
                    s.semantic.is_some_and(|id| {
                        a[..*i]
                            .iter()
                            .all(|p| p.semantic.is_none_or(|v| v.operation != id.operation))
                    })
                })
                .count(),
            request_id_bytes: a.iter().map(|s| s.request_id.len()).sum(),
            basename_bytes: self
                .captures
                .iter()
                .map(String::len)
                .chain(self.streams.iter().map(|s| s.basename.len()))
                .chain(a.iter().map(|s| s.basename.len()))
                .sum(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpeechCounts {
    pub characters: usize,
    pub captures: usize,
    pub streams: usize,
    pub available_texts: usize,
    pub available_text_bytes: usize,
    pub accepted_recordings: usize,
    pub batch_pending: usize,
    pub parked: usize,
    pub semantic_receipts: usize,
    pub terminal_receipts: usize,
    pub unique_roots: usize,
    pub request_id_bytes: usize,
    pub basename_bytes: usize,
}
#[derive(Debug, Serialize)]
pub struct SpeechRouterDtoV1 {
    version: u16,
    boundary: LogicalTime,
    pub(crate) state: StateV1,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    state: StateV1,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    state: StateView<'a>,
}
#[derive(Debug)]
pub struct SpeechCandidate {
    data: SpeechRouterDtoV1,
}
impl SpeechRouter {
    pub fn checkpoint_cost(
        &self,
        c: SpeechCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<SpeechCost>> {
        check(
            self.interrupted.is_empty(),
            "interrupted speech requires complete V2",
        )?;
        let v = View {
            version: 1,
            boundary: c.now,
            state: StateView::new(self, c),
        };
        let cost = prepare(&v, &mut r)?;
        v.state.validate()?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_checkpoint(
        &self,
        c: SpeechCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<SpeechRouterDtoV1>> {
        check(
            self.interrupted.is_empty(),
            "interrupted speech requires complete V2",
        )?;
        let v = View {
            version: 1,
            boundary: c.now,
            state: StateView::new(self, c),
        };
        prepare(&v, &mut r)?;
        v.state.validate()?;
        let d = SpeechRouterDtoV1 {
            version: 1,
            boundary: c.now,
            state: v.state.copy(),
        };
        d.state.validate(c)?;
        Ok(Admitted::new(d, r))
    }
}
impl SpeechRouterDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: SpeechCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            state: w.state,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: SpeechCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported speech version")?;
        binding(self.boundary, c)?;
        self.state.validate(c)
    }
    pub fn cost(&self) -> Result<SpeechCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: SpeechCheckpointContext<'_>) -> SpeechCounts {
        self.state.counts(c)
    }
}
impl Admitted<SpeechRouterDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: SpeechCheckpointContext<'_>,
    ) -> Result<Admitted<SpeechCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(SpeechCandidate { data: d })
        })
    }
}
impl SpeechCandidate {
    pub fn captures(&self) -> &[String] {
        &self.data.state.captures
    }
    pub fn streams(&self) -> &[InterruptedStream] {
        &self.data.state.streams
    }
    pub fn accepted_recordings(&self) -> &[AcceptedRecording] {
        &self.data.state.accepted_recordings
    }
    pub fn stt_stream_grace_seconds(&self) -> f64 {
        self.data.state.stt_stream_grace_seconds
    }
    pub fn purpose(&self) -> InputPurpose {
        self.data.state.purpose
    }
    pub fn status(&self) -> InterruptionStatus {
        self.data.state.status
    }
    pub fn semantic_ids(&self) -> impl Iterator<Item = CommandId> + '_ {
        self.data
            .state
            .accepted_recordings
            .iter()
            .filter_map(|r| r.semantic)
    }
    pub fn counts(&self, c: SpeechCheckpointContext<'_>) -> SpeechCounts {
        self.data.counts(c)
    }
}
#[cfg(test)]
mod tests;
