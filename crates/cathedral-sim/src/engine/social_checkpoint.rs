//! Exact existing social owners and original Engine idle configuration.
//! Full scheduler/speech/envelope adoption and DefaultHasher binding remain pending.
use super::*;
pub use crate::checkpoint::social::SocialCost;
use crate::{
    attention::checkpoint as attention_owner,
    checkpoint::{Admitted, Reservation, Result, aggregate, social},
    conversation::{Conversation, checkpoint as conversation_owner},
    timeline::LogicalTime,
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
use serde::{Deserialize, Serialize};
mod records;
#[derive(Clone, Copy)]
pub struct SocialCheckpointContext<'a> {
    now: LogicalTime,
    backbone: BackboneRefs<'a>,
    player: &'a ActorId,
}
impl<'a> SocialCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            player,
        }
    }
    pub fn from_backbone(b: &'a BackboneCandidate, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            now,
            backbone: b.references(),
            player,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SocialCounts {
    pub characters: usize,
    #[serde(flatten)]
    pub conversation: conversation_owner::ConversationCounts,
    pub warm_pairs: usize,
    pub novelty_memories: usize,
    pub novelty_told: usize,
}
#[derive(Debug, Serialize)]
pub struct EngineSocialDtoV1 {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "conversation_owner::records::ConversationV1")]
    conversation: Conversation,
    #[serde(with = "attention_owner::records::WarmExchangesV1")]
    warm_exchanges: WarmExchanges,
    #[serde(with = "attention_owner::records::NoveltyV1")]
    novelty: Novelty,
    #[serde(with = "records::IdleModeV1")]
    idle_mode: IdleCognitionMode,
    #[serde(with = "records::StageV1")]
    stage: StageConfig,
    idle_requires_news: bool,
    #[serde(with = "records::CuriosityV1")]
    idle_curiosity: CuriosityConfig,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "conversation_owner::records::ConversationV1")]
    conversation: Conversation,
    #[serde(with = "attention_owner::records::WarmExchangesV1")]
    warm_exchanges: WarmExchanges,
    #[serde(with = "attention_owner::records::NoveltyV1")]
    novelty: Novelty,
    #[serde(with = "records::IdleModeV1")]
    idle_mode: IdleCognitionMode,
    #[serde(with = "records::StageV1")]
    stage: StageConfig,
    idle_requires_news: bool,
    #[serde(with = "records::CuriosityV1")]
    idle_curiosity: CuriosityConfig,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    player_id: &'a ActorId,
    #[serde(with = "conversation_owner::records::ConversationV1")]
    conversation: &'a Conversation,
    #[serde(with = "attention_owner::records::WarmExchangesV1")]
    warm_exchanges: &'a WarmExchanges,
    #[serde(with = "attention_owner::records::NoveltyV1")]
    novelty: &'a Novelty,
    #[serde(with = "records::IdleModeV1")]
    idle_mode: IdleCognitionMode,
    #[serde(with = "records::StageV1")]
    stage: StageConfig,
    idle_requires_news: bool,
    #[serde(with = "records::CuriosityV1")]
    idle_curiosity: CuriosityConfig,
}
impl<'a> View<'a> {
    fn new(e: &'a Engine, now: LogicalTime) -> Self {
        Self {
            version: 1,
            boundary: now,
            player_id: &e.config.player_id,
            conversation: &e.conversation,
            warm_exchanges: &e.npc_exchanges,
            novelty: &e.novelty,
            idle_mode: e.config.idle_mode,
            stage: e.config.stage,
            idle_requires_news: e.config.idle_requires_news,
            idle_curiosity: e.config.idle_curiosity,
        }
    }
}
#[derive(Debug)]
pub struct EngineSocialCandidate {
    data: EngineSocialDtoV1,
}
impl Engine {
    pub fn social_checkpoint_context(&self, now: LogicalTime) -> SocialCheckpointContext<'_> {
        SocialCheckpointContext::from_world(&self.world, now, &self.config.player_id)
    }
    pub fn checkpoint_social_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<SocialCost>> {
        let cost = social::prepare(&View::new(self, now), &mut r)?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_social_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineSocialDtoV1>> {
        social::prepare(&View::new(self, now), &mut r)?;
        // Validate the borrowed owners before cloning; no constructor or poll.
        validate_owners(&self.conversation, &self.npc_exchanges, &self.novelty)?;
        validate_binding(
            now,
            now,
            &self.config.player_id,
            self.social_checkpoint_context(now),
        )?;
        let d = EngineSocialDtoV1 {
            version: 1,
            boundary: now,
            player_id: self.config.player_id.clone(),
            conversation: self.conversation.clone(),
            warm_exchanges: self.npc_exchanges.clone(),
            novelty: self.novelty.clone(),
            idle_mode: self.config.idle_mode,
            stage: self.config.stage,
            idle_requires_news: self.config.idle_requires_news,
            idle_curiosity: self.config.idle_curiosity,
        };
        Ok(Admitted::new(d, r))
    }
}
fn validate_owners(c: &Conversation, w: &WarmExchanges, n: &Novelty) -> Result<()> {
    conversation_owner::validate(c)?;
    attention_owner::validate_warm(w)?;
    attention_owner::validate_novelty(n)
}
fn validate_binding(
    boundary: LogicalTime,
    now: LogicalTime,
    player: &ActorId,
    c: SocialCheckpointContext<'_>,
) -> Result<()> {
    crate::checkpoint::logical("social", now.seconds())?;
    social::check(
        boundary.seconds().to_bits() == now.seconds().to_bits(),
        "Engine social boundary disagreement",
    )?;
    social::id(player)?;
    social::check(
        player == c.player && c.backbone.characters.contains_key(player),
        "Engine social player binding disagreement",
    )
}
impl EngineSocialDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: SocialCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire = aggregate::decode_with_working(
            bytes,
            "social",
            &mut r,
            social::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            player_id: w.player_id,
            conversation: w.conversation,
            warm_exchanges: w.warm_exchanges,
            novelty: w.novelty,
            idle_mode: w.idle_mode,
            stage: w.stage,
            idle_requires_news: w.idle_requires_news,
            idle_curiosity: w.idle_curiosity,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: SocialCheckpointContext<'_>) -> Result<()> {
        social::check(self.version == 1, "unsupported Engine social version")?;
        validate_binding(self.boundary, c.now, &self.player_id, c)?;
        validate_owners(&self.conversation, &self.warm_exchanges, &self.novelty)
    }
    pub fn cost(&self) -> Result<SocialCost> {
        Ok(aggregate::measure(self, "social")?.into())
    }
    pub fn counts(&self, c: SocialCheckpointContext<'_>) -> SocialCounts {
        let (warm_pairs, novelty_memories, novelty_told) =
            attention_owner::counts(&self.warm_exchanges, &self.novelty);
        SocialCounts {
            characters: c.backbone.characters.len(),
            conversation: conversation_owner::counts(&self.conversation),
            warm_pairs,
            novelty_memories,
            novelty_told,
        }
    }
}
impl Admitted<EngineSocialDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, "social", r))
    }
    pub fn into_candidate(
        self,
        c: SocialCheckpointContext<'_>,
    ) -> Result<Admitted<EngineSocialCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineSocialCandidate { data: d })
        })
    }
}
impl EngineSocialCandidate {
    pub fn conversation(&self) -> &Conversation {
        &self.data.conversation
    }
    pub fn warm_exchanges(&self) -> &WarmExchanges {
        &self.data.warm_exchanges
    }
    pub fn novelty(&self) -> &Novelty {
        &self.data.novelty
    }
    pub fn idle_mode(&self) -> IdleCognitionMode {
        self.data.idle_mode
    }
    pub fn stage(&self) -> StageConfig {
        self.data.stage
    }
    pub fn idle_requires_news(&self) -> bool {
        self.data.idle_requires_news
    }
    pub fn idle_curiosity(&self) -> CuriosityConfig {
        self.data.idle_curiosity
    }
    pub fn player_id(&self) -> &ActorId {
        &self.data.player_id
    }
    pub fn counts(&self, c: SocialCheckpointContext<'_>) -> SocialCounts {
        self.data.counts(c)
    }
}
#[cfg(test)]
mod tests;
