//! Exact private social authority; candidates are read-only and never installed in production.
use super::*;
pub use crate::checkpoint::social::SocialCost;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate, social},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub(crate) mod records;
use records::*;
pub(crate) fn validate(v: &Conversation) -> Result<()> {
    social::check(
        v.latest_applied_utterance <= v.next_utterance,
        "conversation token order",
    )?;
    if let Some(e) = &v.engagement {
        social::id(&e.actor)?;
        social::anchor(e.at)?;
        social::check(
            e.witnesses.len() <= social::MAX_WITNESSES,
            "conversation witness count",
        )?;
        for a in &e.witnesses {
            social::id(a)?;
        }
    }
    if let Some((a, at)) = &v.invitation {
        social::id(a)?;
        social::anchor(*at)?;
    }
    if let Some((a, since, last)) = &v.focus {
        social::id(a)?;
        social::anchor(*since)?;
        social::anchor(*last)?;
    }
    // Historical valid identities need not remain in World; a target need not
    // be a witness. Backwards public samples permit since > last. No pruning.
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConversationCounts {
    pub engaged: bool,
    pub reciprocal: bool,
    pub witnesses: usize,
    pub invitation: bool,
    pub focus: bool,
    pub next_utterance: u64,
    pub latest_applied_utterance: u64,
}
pub(crate) fn counts(v: &Conversation) -> ConversationCounts {
    ConversationCounts {
        engaged: v.engagement.is_some(),
        reciprocal: v.engagement.as_ref().is_some_and(|e| e.reciprocal),
        witnesses: v.engagement.as_ref().map_or(0, |e| e.witnesses.len()),
        invitation: v.invitation.is_some(),
        focus: v.focus.is_some(),
        next_utterance: v.next_utterance,
        latest_applied_utterance: v.latest_applied_utterance,
    }
}

#[derive(Debug, Serialize)]
pub struct ConversationDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "ConversationV1")]
    state: Conversation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationWire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "ConversationV1")]
    state: Conversation,
}
#[derive(Serialize)]
struct ConversationView<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "ConversationV1")]
    state: &'a Conversation,
}
#[derive(Debug)]
pub struct ConversationCandidate {
    data: ConversationDtoV1,
}
impl Conversation {
    pub fn export_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<ConversationDtoV1>> {
        social::prepare(
            &ConversationView {
                version: 1,
                boundary: now,
                state: self,
            },
            &mut r,
        )?;
        crate::checkpoint::logical("social", now.seconds())?;
        validate(self)?;
        Ok(Admitted::new(
            ConversationDtoV1 {
                version: 1,
                boundary: now,
                state: self.clone(),
            },
            r,
        ))
    }
}
impl ConversationDtoV1 {
    pub fn decode(bytes: &[u8], mut r: Reservation, now: LogicalTime) -> Result<Admitted<Self>> {
        let w: ConversationWire = aggregate::decode_with_working(
            bytes,
            "social",
            &mut r,
            social::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            state: w.state,
        };
        d.validate(now)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, now: LogicalTime) -> Result<()> {
        crate::checkpoint::logical("social", now.seconds())?;
        social::check(self.version == 1, "unsupported social version")?;
        social::check(
            self.boundary.seconds().to_bits() == now.seconds().to_bits(),
            "social boundary disagreement",
        )?;
        validate(&self.state)
    }
    pub fn cost(&self) -> Result<SocialCost> {
        Ok(aggregate::measure(self, "social")?.into())
    }
}
impl Admitted<ConversationDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, "social", r))
    }
    pub fn into_candidate(self, now: LogicalTime) -> Result<Admitted<ConversationCandidate>> {
        self.try_map(|d, _| {
            d.validate(now)?;
            Ok(ConversationCandidate { data: d })
        })
    }
}
impl ConversationCandidate {
    pub fn conversation(&self) -> &Conversation {
        &self.data.state
    }
}

#[cfg(test)]
mod tests;
