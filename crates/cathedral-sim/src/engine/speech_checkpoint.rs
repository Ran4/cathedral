//! Read-only Engine composition of interrupted speech input. Full M2c must
//! terminalize outstanding receipts, release roots and clear old Floor holds
//! together; M3 owns generation-fenced services and initial host publication.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate},
    receipts::{CommandId, CommandLedgerDtoV1},
    speech_router::checkpoint::{
        self as owner, SpeechCheckpointContext, StateView, records::StateV1,
    },
    timeline::LogicalTime,
    world::checkpoint::BackboneCandidate,
};
pub use owner::{
    AcceptedRecording, InputPurpose, InterruptedStream, InterruptionStatus, SpeechCost,
    SpeechCounts,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy)]
pub struct EngineSpeechCheckpointContext<'a> {
    speech: SpeechCheckpointContext<'a>,
    player: &'a ActorId,
}
impl<'a> EngineSpeechCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            speech: SpeechCheckpointContext::from_world(w, now),
            player,
        }
    }
    pub fn from_backbone(
        b: &'a BackboneCandidate,
        now: LogicalTime,
        ledger: &'a CommandLedgerDtoV1,
        player: &'a ActorId,
    ) -> Self {
        Self {
            speech: SpeechCheckpointContext::from_backbone(b, now, ledger),
            player,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct EngineSpeechDtoV1 {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    state: StateV1,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    state: StateV1,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    player_id: &'a ActorId,
    state: StateView<'a>,
}
#[derive(Debug)]
pub struct EngineSpeechCandidate {
    data: EngineSpeechDtoV1,
    interrupted: Vec<owner::InterruptedSpeech>,
}
fn binding(
    boundary: LogicalTime,
    player: &ActorId,
    c: EngineSpeechCheckpointContext<'_>,
) -> Result<()> {
    owner::binding(boundary, c.speech)?;
    owner::check(
        player.as_str().len() <= 4 * crate::MAX_ID_CHARS
            && player.is_valid()
            && player == c.player
            && c.speech.backbone.characters.contains_key(player),
        "Engine speech player binding disagreement",
    )
}
impl Engine {
    pub fn speech_checkpoint_context(&self, now: LogicalTime) -> EngineSpeechCheckpointContext<'_> {
        EngineSpeechCheckpointContext::from_world(&self.world, now, &self.config.player_id)
    }
    pub fn checkpoint_speech_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<SpeechCost>> {
        owner::check(
            self.speech_router.interrupted.is_empty(),
            "interrupted speech requires complete V2",
        )?;
        let c = self.speech_checkpoint_context(now);
        let v = View {
            version: 1,
            boundary: now,
            player_id: &self.config.player_id,
            state: StateView::new(&self.speech_router, c.speech),
        };
        let cost = owner::prepare(&v, &mut r)?;
        binding(now, v.player_id, c)?;
        v.state.validate()?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_speech_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineSpeechDtoV1>> {
        owner::check(
            self.speech_router.interrupted.is_empty(),
            "interrupted speech requires complete V2",
        )?;
        let c = self.speech_checkpoint_context(now);
        let v = View {
            version: 1,
            boundary: now,
            player_id: &self.config.player_id,
            state: StateView::new(&self.speech_router, c.speech),
        };
        owner::prepare(&v, &mut r)?;
        binding(now, v.player_id, c)?;
        v.state.validate()?;
        let d = EngineSpeechDtoV1 {
            version: 1,
            boundary: now,
            player_id: self.config.player_id.clone(),
            state: v.state.copy(),
        };
        d.state.validate(c.speech)?;
        Ok(Admitted::new(d, r))
    }
}
impl EngineSpeechDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: EngineSpeechCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire = aggregate::decode_with_working(
            bytes,
            owner::OWNER,
            &mut r,
            owner::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            player_id: w.player_id,
            state: w.state,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: EngineSpeechCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine speech version")?;
        binding(self.boundary, &self.player_id, c)?;
        self.state.validate(c.speech)
    }
    pub fn cost(&self) -> Result<SpeechCost> {
        Ok(aggregate::measure(self, owner::OWNER)?.into())
    }
    pub fn counts(&self, c: EngineSpeechCheckpointContext<'_>) -> SpeechCounts {
        self.state.counts(c.speech)
    }
}
impl Admitted<EngineSpeechDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, owner::OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: EngineSpeechCheckpointContext<'_>,
    ) -> Result<Admitted<EngineSpeechCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineSpeechCandidate {
                data: d,
                interrupted: Vec::new(),
            })
        })
    }
}
impl EngineSpeechCandidate {
    pub fn player_id(&self) -> &ActorId {
        &self.data.player_id
    }
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
            .filter_map(|r| r.semantic())
    }
    pub fn counts(&self, c: EngineSpeechCheckpointContext<'_>) -> SpeechCounts {
        self.data.counts(c)
    }
}
#[cfg(test)]
mod tests;

// Closed full-envelope decoding: the shared meter reserves before every
// typed allocation. This function never creates an independent component budget.
impl EngineSpeechDtoV1 {
    pub(crate) fn complete_decode(
        bytes: &[u8],
        meter: &crate::checkpoint::complete::meter::DecodeMeter<'_>,
        c: EngineSpeechCheckpointContext<'_>,
    ) -> Result<EngineSpeechCandidate> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct V2 {
            version: u16,
            base: Wire,
            interrupted: Vec<owner::InterruptedSpeech>,
        }
        meter.prepare_diagnostics(bytes)?;
        let (w, interrupted) = match crate::checkpoint::complete::owner_version(bytes)? {
            1 => (meter.decode::<Wire>(bytes)?, Vec::new()),
            2 => {
                let v: V2 = meter.decode(bytes)?;
                owner::check(v.version == 2, "unsupported speech extension")?;
                (v.base, v.interrupted)
            }
            _ => {
                return Err(crate::checkpoint::CheckpointError::new(
                    "speech",
                    "unsupported speech complete version",
                ));
            }
        };
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            player_id: w.player_id,
            state: w.state,
        };
        d.validate(c)?;
        owner::validate_interrupted(&interrupted, c.speech)?;
        owner::validate_active_overlap(
            &interrupted,
            d.state
                .accepted_recordings
                .iter()
                .filter_map(|t| t.semantic()),
        )?;
        Ok(EngineSpeechCandidate {
            data: d,
            interrupted,
        })
    }
}

impl Engine {
    pub(crate) fn complete_write_speech<W: std::io::Write>(
        &self,
        now: LogicalTime,
        writer: &mut W,
        r: &mut Reservation,
    ) -> Result<()> {
        r.require(
            crate::checkpoint::Cohort::SavePayload,
            crate::checkpoint::complete::meter::VALIDATION_SCRATCH,
        )?;
        let c = self.speech_checkpoint_context(now);
        let view = View {
            version: 1,
            boundary: now,
            player_id: &self.config.player_id,
            state: StateView::new(&self.speech_router, c.speech),
        };
        view.state.validate()?;
        owner::validate_interrupted(&self.speech_router.interrupted, c.speech)?;
        write_complete(writer, &view, &self.speech_router.interrupted)
    }
}

impl EngineSpeechCandidate {
    pub(crate) fn complete_write_retained<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        write_complete(writer, &self.data, &self.interrupted)
    }
}

fn write_complete<W: std::io::Write, T: Serialize>(
    writer: &mut W,
    base: &T,
    interrupted: &[owner::InterruptedSpeech],
) -> Result<()> {
    if interrupted.is_empty() {
        crate::checkpoint::complete::write_json(writer, base)
    } else {
        #[derive(Serialize)]
        struct V2<'a, T: Serialize> {
            version: u16,
            base: &'a T,
            interrupted: &'a [owner::InterruptedSpeech],
        }
        crate::checkpoint::complete::write_json(
            writer,
            &V2 {
                version: 2,
                base,
                interrupted,
            },
        )
    }
}

impl EngineSpeechCandidate {
    pub(crate) fn continuation_preflight(&self) -> Result<()> {
        let existing: usize = self
            .interrupted
            .iter()
            .map(owner::InterruptedSpeech::count)
            .sum();
        let s = &self.data.state;
        owner::check(
            existing + s.captures.len() + s.streams.len() + s.accepted_recordings.len()
                <= owner::MAX_INTERRUPTED_INPUTS,
            "interrupted input count limit",
        )
    }
    #[allow(dead_code)] // Compatibility wrapper; the host uses retained preparation.
    pub(crate) fn prepare_continuation(
        self,
        world: &mut World,
        router: &mut SpeechRouter,
        now: LogicalTime,
    ) -> Result<()> {
        self.prepare_continuation_retained(world, router, now)
            .map_err(|(error, _)| error)
    }
    pub(crate) fn prepare_continuation_retained(
        self,
        world: &mut World,
        router: &mut SpeechRouter,
        now: LogicalTime,
    ) -> std::result::Result<(), (crate::checkpoint::CheckpointError, Self)> {
        // Every accepted command was already validated against the ledger at
        // this boundary. Advance never repeats an already committed effect.
        for task in &self.data.state.accepted_recordings {
            if let Some(id) = task.semantic() {
                if let Err(e) = world.command_ledger.advance(
                    id,
                    now.seconds(),
                    crate::receipts::Outcome::new(
                        crate::receipts::ReceiptState::Interrupted,
                        owner::INTERRUPTION_CODE,
                        owner::INTERRUPTION_MESSAGE,
                    ),
                ) {
                    return Err((
                        crate::checkpoint::CheckpointError::new("speech", e.message),
                        self,
                    ));
                }
                world.speech_actions.remove(&id);
                crate::receipts::release_finished_root(world, id.operation);
            }
        }
        let receipts = world.command_ledger.drain_updates();
        router.interrupted = self.interrupted;
        let state = self.data.state;
        if !state.captures.is_empty()
            || !state.streams.is_empty()
            || !state.accepted_recordings.is_empty()
        {
            router
                .interrupted
                .push(owner::InterruptedSpeech::new(now, state, receipts));
        }
        Ok(())
    }
}
