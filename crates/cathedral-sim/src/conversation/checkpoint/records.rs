#![allow(private_interfaces)]
use super::*;
use crate::checkpoint::serde_support::{remote_adapters, unique_set};
#[derive(Serialize, Deserialize)]
#[serde(remote = "Engagement", deny_unknown_fields)]
pub(crate) struct EngagementV1 {
    actor: ActorId,
    at: f64,
    reciprocal: bool,
    #[serde(with = "unique_set")]
    witnesses: BTreeSet<ActorId>,
}
remote_adapters!(engagement, Engagement, EngagementV1);
pub(crate) mod invitation {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row<A> {
        actor: A,
        at: f64,
    }
    pub fn serialize<S: serde::Serializer>(
        v: &Option<(ActorId, f64)>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.as_ref()
            .map(|(actor, at)| Row { actor, at: *at })
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Option<(ActorId, f64)>, D::Error> {
        Ok(Option::<Row<ActorId>>::deserialize(d)?.map(|r| (r.actor, r.at)))
    }
}
pub(crate) mod focus {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row<A> {
        actor: A,
        since: f64,
        last: f64,
    }
    pub fn serialize<S: serde::Serializer>(
        v: &Option<(ActorId, f64, f64)>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.as_ref()
            .map(|(actor, since, last)| Row {
                actor,
                since: *since,
                last: *last,
            })
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Option<(ActorId, f64, f64)>, D::Error> {
        Ok(Option::<Row<ActorId>>::deserialize(d)?.map(|r| (r.actor, r.since, r.last)))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Conversation", deny_unknown_fields)]
pub(crate) struct ConversationV1 {
    #[serde(with = "engagement::option")]
    engagement: Option<Engagement>,
    #[serde(with = "invitation")]
    invitation: Option<(ActorId, f64)>,
    #[serde(with = "focus")]
    focus: Option<(ActorId, f64, f64)>,
    next_utterance: u64,
    latest_applied_utterance: u64,
}
