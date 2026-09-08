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
            Ok(EngineSpeechCandidate { data: d })
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
