//! Exhaustive command policy and version-one semantic payload projection.
//! Request IDs remain UI correlation only. Every consequential variant enters
//! the shared ledger, including controls whose old wire had no result reply.
use super::EngineCommand;
use crate::receipts::{AffectedRef, CommandId};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPolicy {
    Consequential,
    LatestPosition,
    Attention,
    SpeechLifecycle,
    ProviderCompletion,
}

impl EngineCommand {
    pub fn identified(self, id: CommandId) -> Self {
        Self::Identified {
            id,
            command: Box::new(self),
        }
    }

    pub fn policy(&self) -> CommandPolicy {
        match self {
            Self::Identified { command, .. } => command.policy(),
            Self::SpatialUpdate { .. } => CommandPolicy::LatestPosition,
            Self::PlayerOffer { .. } => CommandPolicy::Consequential,
            Self::PlayerAccept { .. } => CommandPolicy::Consequential,
            Self::PlayerDecline { .. } => CommandPolicy::Consequential,
            Self::PlayerRetract { .. } => CommandPolicy::Consequential,
            Self::PlayerPocket { .. } => CommandPolicy::Consequential,
            Self::PlayerRetrieve { .. } => CommandPolicy::Consequential,
            Self::PlayerSwallow { .. } => CommandPolicy::Consequential,
            Self::PlayerSpit { .. } => CommandPolicy::Consequential,
            Self::PlayerGargle { .. } => CommandPolicy::Consequential,
            Self::PlayerExpel { .. } => CommandPolicy::Consequential,
            Self::PlayerEat { .. } => CommandPolicy::Consequential,
            Self::PlayerGrabbed { .. } => CommandPolicy::Consequential,
            Self::PlayerStruggling => CommandPolicy::Consequential,
            Self::PlayerBrokeFree => CommandPolicy::Consequential,
            Self::PlayerSound { .. } => CommandPolicy::Consequential,
            Self::DebugSound { .. } => CommandPolicy::Consequential,
            Self::WorldSound { .. } => CommandPolicy::Consequential,
            Self::Knell { .. } => CommandPolicy::Consequential,
            Self::CivicPeal { .. } => CommandPolicy::Consequential,
            Self::DebugSetStatus { .. } => CommandPolicy::Consequential,
            Self::DebugSeize { .. } => CommandPolicy::Consequential,
            Self::DebugOwe { .. } => CommandPolicy::Consequential,
            Self::DebugChalk { .. } => CommandPolicy::Consequential,
            Self::DebugSeedFact { .. } => CommandPolicy::Consequential,
            Self::DebugRaiseWord { .. } => CommandPolicy::Consequential,
            Self::DebugScrub { .. } => CommandPolicy::Consequential,
            Self::PlayerScrubMark { .. } => CommandPolicy::Consequential,
            Self::PlayerDrawMark { .. } => CommandPolicy::Consequential,
            Self::DebugCommit { .. } => CommandPolicy::Consequential,
            Self::CycleTimeScale => CommandPolicy::Consequential,
            Self::SetWeatherOverride { .. } => CommandPolicy::Consequential,
            Self::ClearWeatherOverride => CommandPolicy::Consequential,
            Self::DebugPlayerSay { .. } => CommandPolicy::Consequential,
            Self::PlayerSay { .. } => CommandPolicy::Consequential,
            Self::PlayerAttention { .. } => CommandPolicy::Attention,
            Self::PlayerUtteranceStarted { .. } => CommandPolicy::SpeechLifecycle,
            Self::SpeechPresented { .. } => CommandPolicy::SpeechLifecycle,
            Self::SetTtsBackend { .. } => CommandPolicy::Consequential,
            Self::PlayerRecording { .. } => CommandPolicy::Consequential,
            Self::PlayerAudioBegin { .. } => CommandPolicy::SpeechLifecycle,
            Self::PlayerAudioChunk { .. } => CommandPolicy::SpeechLifecycle,
            Self::PlayerAudioEnd { .. } => CommandPolicy::SpeechLifecycle,
            Self::PlayerAudioAbort { .. } => CommandPolicy::SpeechLifecycle,
            Self::LlmCompletion(..) => CommandPolicy::ProviderCompletion,
            Self::Transcription(..) => CommandPolicy::ProviderCompletion,
            Self::Tts(..) => CommandPolicy::ProviderCompletion,
            Self::BackendStatus(..) => CommandPolicy::ProviderCompletion,
        }
    }

    pub(super) fn payload(&self) -> Option<Value> {
        let payload = match self {
            Self::Identified { command, .. } => return command.payload(),
            Self::SpatialUpdate { .. } => return None,
            Self::PlayerOffer {
                item_id,
                target_id,
                quantity,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "PlayerOffer", "item_id": item_id, "target_id": target_id, "quantity": quantity, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerAccept {
                item_id,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "PlayerAccept", "item_id": item_id, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerDecline {
                item_id,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "PlayerDecline", "item_id": item_id, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerRetract { item_id, .. } => {
                json!({"command": "PlayerRetract", "item_id": item_id})
            }
            Self::PlayerPocket { item_id, slot, .. } => {
                json!({"command": "PlayerPocket", "item_id": item_id, "slot": slot})
            }
            Self::PlayerRetrieve { item_id, .. } => {
                json!({"command": "PlayerRetrieve", "item_id": item_id})
            }
            Self::PlayerSwallow { item_id, .. } => {
                json!({"command": "PlayerSwallow", "item_id": item_id})
            }
            Self::PlayerSpit {
                item_id,
                target_id,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "PlayerSpit", "item_id": item_id, "target_id": target_id, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerGargle { item_id, .. } => {
                json!({"command": "PlayerGargle", "item_id": item_id})
            }
            Self::PlayerExpel { .. } => json!({"command": "PlayerExpel"}),
            Self::PlayerEat { item_id, .. } => json!({"command": "PlayerEat", "item_id": item_id}),
            Self::PlayerGrabbed { holder_id, .. } => {
                json!({"command": "PlayerGrabbed", "holder_id": holder_id})
            }
            Self::PlayerStruggling => json!({"command": "PlayerStruggling"}),
            Self::PlayerBrokeFree => json!({"command": "PlayerBrokeFree"}),
            Self::PlayerSound { sound_id, .. } => {
                json!({"command": "PlayerSound", "sound_id": sound_id})
            }
            Self::DebugSound {
                sound_id,
                position_m,
                ..
            } => {
                json!({"command": "DebugSound", "sound_id": sound_id, "position_m": position_m.to_array()})
            }
            Self::WorldSound {
                sound_id,
                position_m,
                ..
            } => {
                json!({"command": "WorldSound", "sound_id": sound_id, "position_m": position_m.to_array()})
            }
            Self::Knell { years, at, .. } => {
                json!({"command": "Knell", "years": years, "at": at.to_array()})
            }
            Self::CivicPeal {
                rope, at, radius_m, ..
            } => {
                json!({"command": "CivicPeal", "rope": match rope { super::CivicRope::Curfew => "curfew", super::CivicRope::Summons => "summons" }, "at": at.to_array(), "radius_m": radius_m})
            }
            Self::DebugSetStatus {
                name, kind, value, ..
            } => json!({"command": "DebugSetStatus", "name": name, "kind": kind, "value": value}),
            Self::DebugSeize {
                officer, target, ..
            } => json!({"command": "DebugSeize", "officer": officer, "target": target}),
            Self::DebugOwe { who, .. } => json!({"command": "DebugOwe", "who": who}),
            Self::DebugChalk { kind, anchor, .. } => {
                json!({"command": "DebugChalk", "kind": kind, "anchor": anchor})
            }
            Self::DebugSeedFact { fact, ward, .. } => {
                json!({"command": "DebugSeedFact", "fact": fact, "ward": ward})
            }
            Self::DebugRaiseWord {
                who, topic, said, ..
            } => json!({"command": "DebugRaiseWord", "who": who, "topic": topic, "said": said}),
            Self::DebugScrub { anchor, .. } => json!({"command": "DebugScrub", "anchor": anchor}),
            Self::PlayerScrubMark { mark_id, .. } => {
                json!({"command": "PlayerScrubMark", "mark_id": mark_id})
            }
            Self::PlayerDrawMark { kind, anchor, .. } => {
                json!({"command": "PlayerDrawMark", "kind": kind, "anchor": anchor})
            }
            Self::DebugCommit { target, .. } => json!({"command": "DebugCommit", "target": target}),
            Self::CycleTimeScale => json!({"command": "CycleTimeScale"}),
            Self::SetWeatherOverride {
                kind, intensity, ..
            } => json!({"command": "SetWeatherOverride", "kind": kind, "intensity": intensity}),
            Self::ClearWeatherOverride => json!({"command": "ClearWeatherOverride"}),
            Self::DebugPlayerSay {
                text,
                target_id,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "DebugPlayerSay", "text": text, "target_id": target_id, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerSay {
                text,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command": "PlayerSay", "text": text, "position_m": position_m.to_array(), "spatial_seq": spatial_seq})
            }
            Self::PlayerAttention { .. } => return None,
            Self::PlayerUtteranceStarted { .. } => return None,
            Self::SpeechPresented { .. } => return None,
            Self::SetTtsBackend { backend, .. } => {
                json!({"command": "SetTtsBackend", "backend": backend})
            }
            Self::PlayerRecording {
                wav_basename,
                stt_backend,
                position_m,
                spatial_seq,
                ..
            } => {
                json!({"command":"player_recording","basename":wav_basename,"backend":stt_backend.as_str(),"position":position_m.to_array(),"spatial_seq":spatial_seq})
            }
            Self::PlayerAudioBegin { .. } => return None,
            Self::PlayerAudioChunk { .. } => return None,
            Self::PlayerAudioEnd { .. } => return None,
            Self::PlayerAudioAbort { .. } => return None,
            Self::LlmCompletion(..) => return None,
            Self::Transcription(..) => return None,
            Self::Tts(..) => return None,
            Self::BackendStatus(..) => return None,
        };
        Some(payload)
    }

    pub(super) fn correlation(&self) -> Option<&str> {
        match self {
            Self::Identified { command, .. } => command.correlation(),
            Self::PlayerOffer { request_id, .. } => Some(request_id),
            Self::PlayerAccept { request_id, .. } => Some(request_id),
            Self::PlayerDecline { request_id, .. } => Some(request_id),
            Self::PlayerRetract { request_id, .. } => Some(request_id),
            Self::PlayerPocket { request_id, .. } => Some(request_id),
            Self::PlayerRetrieve { request_id, .. } => Some(request_id),
            Self::PlayerSwallow { request_id, .. } => Some(request_id),
            Self::PlayerSpit { request_id, .. } => Some(request_id),
            Self::PlayerGargle { request_id, .. } => Some(request_id),
            Self::PlayerExpel { request_id, .. } => Some(request_id),
            Self::PlayerEat { request_id, .. } => Some(request_id),
            Self::DebugPlayerSay { request_id, .. } => Some(request_id),
            Self::PlayerSay { request_id, .. } => Some(request_id),
            Self::SetTtsBackend { request_id, .. } => Some(request_id),
            Self::PlayerRecording { request_id, .. } => Some(request_id),
            _ => None,
        }
    }

    /// Bound borrowed raw payload bytes before json! allocates its projection.
    pub(super) fn bounded_raw_payload(&self) -> bool {
        let variable = match self {
            Self::Identified { command, .. } => return command.bounded_raw_payload(),
            Self::SpatialUpdate { .. } => 0usize,
            Self::PlayerOffer {
                item_id, target_id, ..
            } => 0usize
                .saturating_add(item_id.as_str().len())
                .saturating_add(target_id.as_str().len()),
            Self::PlayerAccept { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerDecline { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerRetract { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerPocket { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerRetrieve { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerSwallow { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerSpit {
                item_id, target_id, ..
            } => 0usize
                .saturating_add(item_id.as_str().len())
                .saturating_add(target_id.as_str().len()),
            Self::PlayerGargle { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerExpel { .. } => 0usize,
            Self::PlayerEat { item_id, .. } => 0usize.saturating_add(item_id.as_str().len()),
            Self::PlayerGrabbed { holder_id } => 0usize.saturating_add(holder_id.as_str().len()),
            Self::PlayerStruggling => 0usize,
            Self::PlayerBrokeFree => 0usize,
            Self::PlayerSound { sound_id } => 0usize.saturating_add(sound_id.len()),
            Self::DebugSound { sound_id, .. } => 0usize.saturating_add(sound_id.len()),
            Self::WorldSound { sound_id, .. } => 0usize.saturating_add(sound_id.len()),
            Self::Knell { .. } => 0usize,
            Self::CivicPeal { .. } => 0usize,
            Self::DebugSetStatus { name, .. } => 0usize.saturating_add(name.len()),
            Self::DebugSeize { officer, target } => 0usize
                .saturating_add(officer.len())
                .saturating_add(target.as_ref().map_or(0, |s| s.len())),
            Self::DebugOwe { who } => 0usize.saturating_add(who.len()),
            Self::DebugChalk { kind, anchor } => 0usize
                .saturating_add(kind.len())
                .saturating_add(anchor.len()),
            Self::DebugSeedFact { fact, ward } => 0usize
                .saturating_add(fact.len())
                .saturating_add(ward.as_ref().map_or(0, |s| s.len())),
            Self::DebugRaiseWord { who, topic, said } => 0usize
                .saturating_add(who.len())
                .saturating_add(topic.len())
                .saturating_add(said.len()),
            Self::DebugScrub { anchor } => 0usize.saturating_add(anchor.len()),
            Self::PlayerScrubMark { .. } => 0usize,
            Self::PlayerDrawMark { anchor, .. } => 0usize.saturating_add(anchor.len()),
            Self::DebugCommit { target } => {
                0usize.saturating_add(target.as_ref().map_or(0, |s| s.len()))
            }
            Self::CycleTimeScale => 0usize,
            Self::SetWeatherOverride { .. } => 0usize,
            Self::ClearWeatherOverride => 0usize,
            Self::DebugPlayerSay {
                text, target_id, ..
            } => 0usize
                .saturating_add(text.len())
                .saturating_add(target_id.as_ref().map_or(0, |id| id.as_str().len())),
            Self::PlayerSay { text, .. } => 0usize.saturating_add(text.len()),
            Self::PlayerAttention { actor_id } => {
                0usize.saturating_add(actor_id.as_ref().map_or(0, |id| id.as_str().len()))
            }
            Self::PlayerUtteranceStarted { wav_basename } => {
                0usize.saturating_add(wav_basename.len())
            }
            Self::SpeechPresented { .. } => 0usize,
            Self::SetTtsBackend { .. } => 0usize,
            Self::PlayerRecording { wav_basename, .. } => 0usize.saturating_add(wav_basename.len()),
            Self::PlayerAudioBegin { wav_basename, .. } => {
                0usize.saturating_add(wav_basename.len())
            }
            Self::PlayerAudioChunk { wav_basename, .. } => {
                0usize.saturating_add(wav_basename.len())
            }
            Self::PlayerAudioEnd { wav_basename, .. } => 0usize.saturating_add(wav_basename.len()),
            Self::PlayerAudioAbort { wav_basename } => 0usize.saturating_add(wav_basename.len()),
            Self::LlmCompletion(..) => 0usize,
            Self::Transcription(..) => 0usize,
            Self::Tts(..) => 0usize,
            Self::BackendStatus(..) => 0usize,
        };
        variable <= crate::receipts::MAX_PAYLOAD_BYTES - 512
    }

    pub(super) fn valid_numbers(&self) -> bool {
        match self {
            Self::Identified { command, .. } => command.valid_numbers(),
            Self::PlayerRecording { position_m, .. } => position_m.is_finite(),
            Self::PlayerOffer { position_m, .. }
            | Self::PlayerAccept { position_m, .. }
            | Self::PlayerDecline { position_m, .. }
            | Self::PlayerSpit { position_m, .. }
            | Self::DebugPlayerSay { position_m, .. }
            | Self::PlayerSay { position_m, .. }
            | Self::DebugSound { position_m, .. }
            | Self::WorldSound { position_m, .. } => position_m.is_finite(),
            Self::Knell { at, .. } => at.is_finite(),
            Self::CivicPeal { at, radius_m, .. } => {
                at.is_finite() && radius_m.is_finite() && *radius_m >= 0.0
            }
            Self::DebugSetStatus { value, .. } => value.is_finite(),
            Self::SetWeatherOverride { intensity, .. } => intensity.is_none_or(f64::is_finite),
            _ => true,
        }
    }

    pub(super) fn affected(&self, world: &crate::world::World) -> Vec<AffectedRef> {
        let actor = |id: &crate::ActorId| AffectedRef::new("actor", id.as_str());
        let item = |id: &crate::ItemId| AffectedRef::new("item", id.as_str());
        match self {
            Self::Identified { command, .. } => command.affected(world),
            Self::PlayerOffer {
                item_id, target_id, ..
            }
            | Self::PlayerSpit {
                item_id, target_id, ..
            } => [item(item_id), actor(target_id)]
                .into_iter()
                .flatten()
                .collect(),
            Self::PlayerAccept { item_id, .. }
            | Self::PlayerDecline { item_id, .. }
            | Self::PlayerRetract { item_id, .. }
            | Self::PlayerPocket { item_id, .. }
            | Self::PlayerRetrieve { item_id, .. }
            | Self::PlayerSwallow { item_id, .. }
            | Self::PlayerGargle { item_id, .. }
            | Self::PlayerEat { item_id, .. } => item(item_id).into_iter().collect(),
            Self::PlayerGrabbed { holder_id } => actor(holder_id).into_iter().collect(),
            Self::DebugSeize { officer, target } => [Some(officer.as_str()), target.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(|handle| world.resolve_debug_handle(handle))
                .filter_map(|id| actor(&id))
                .collect(),
            Self::DebugSetStatus { name, .. }
            | Self::DebugOwe { who: name }
            | Self::DebugRaiseWord { who: name, .. } => world
                .resolve_debug_handle(name)
                .and_then(|id| actor(&id))
                .into_iter()
                .collect(),
            Self::PlayerScrubMark { mark_id } => AffectedRef::new("mark", &mark_id.to_string())
                .into_iter()
                .collect(),
            // A display/anchor handle is not a stable identity. Omit optional
            // unresolved detail rather than manufacture or truncate a ref.
            _ => Vec::new(),
        }
    }
}
