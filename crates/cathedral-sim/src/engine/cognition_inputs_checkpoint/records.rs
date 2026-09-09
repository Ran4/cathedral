use super::*;
use crate::checkpoint::{
    records::{OperationIdV1, TextV1},
    serde_support::required_option,
};
use serde::{Deserialize, Serialize};

/// The exact trait entry point used by the accepted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitionRequestMethod {
    RequestWithBudget,
    RequestNight,
}
struct MethodWire;
impl MethodWire {
    fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<CognitionRequestMethod, D::Error> {
        match TextV1::deserialize(d)?.as_str() {
            "request_with_budget" => Ok(CognitionRequestMethod::RequestWithBudget),
            "request_night" => Ok(CognitionRequestMethod::RequestNight),
            _ => Err(serde::de::Error::custom("invalid cognition request method")),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerInputLane {
    PlayerReaction,
    Handoff,
    Idle,
}
struct LaneWire;
impl LaneWire {
    fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<SchedulerInputLane, D::Error> {
        match TextV1::deserialize(d)?.as_str() {
            "player_reaction" => Ok(SchedulerInputLane::PlayerReaction),
            "handoff" => Ok(SchedulerInputLane::Handoff),
            "idle" => Ok(SchedulerInputLane::Idle),
            _ => Err(serde::de::Error::custom("invalid cognition scheduler lane")),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NightInputSubject {
    Person(ActorId),
    Ward(crate::lore::PlanningWard),
}
#[derive(Deserialize)]
#[serde(
    remote = "NightInputSubject",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum SubjectWire {
    Person(ActorId),
    Ward(#[serde(deserialize_with = "ward")] crate::lore::PlanningWard),
}
fn ward<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<crate::lore::PlanningWard, D::Error> {
    let text = TextV1::deserialize(d)?;
    crate::lore::PlanningWard::ALL
        .into_iter()
        .find(|w| w.as_str() == text)
        .ok_or_else(|| serde::de::Error::custom("invalid cognition Night ward"))
}

/// Accepted scheduler input. Owned only inside its admitted component.
#[derive(Debug, Serialize)]
pub struct SchedulerInput {
    method: CognitionRequestMethod,
    actor_id: ActorId,
    presence_epoch: u64,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    lane: SchedulerInputLane,
    prompt: String,
    output_token_budget: Option<u32>,
}
#[derive(Deserialize)]
#[serde(remote = "SchedulerInput", deny_unknown_fields)]
struct SchedulerWire {
    #[serde(with = "MethodWire")]
    method: CognitionRequestMethod,
    actor_id: ActorId,
    presence_epoch: u64,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    #[serde(with = "LaneWire")]
    lane: SchedulerInputLane,
    #[serde(with = "TextV1")]
    prompt: String,
    #[serde(deserialize_with = "required_option")]
    output_token_budget: Option<u32>,
}
/// Accepted person or ward input. The owed day and incarnation bind its duty.
#[derive(Debug, Serialize)]
pub struct NightInput {
    method: CognitionRequestMethod,
    subject: NightInputSubject,
    presence_epoch: Option<u64>,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    owed_day: i64,
    prompt: String,
    output_token_budget: Option<u32>,
}
#[derive(Deserialize)]
#[serde(remote = "NightInput", deny_unknown_fields)]
struct NightWire {
    #[serde(with = "MethodWire")]
    method: CognitionRequestMethod,
    #[serde(with = "SubjectWire")]
    subject: NightInputSubject,
    #[serde(deserialize_with = "required_option")]
    presence_epoch: Option<u64>,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    owed_day: i64,
    #[serde(with = "TextV1")]
    prompt: String,
    #[serde(deserialize_with = "required_option")]
    output_token_budget: Option<u32>,
}
// The wrapper makes nullable rows mandatory without public Deserialize access.
#[derive(Deserialize)]
struct SchedulerDecoded(#[serde(with = "SchedulerWire")] SchedulerInput);
#[derive(Deserialize)]
struct NightDecoded(#[serde(with = "NightWire")] NightInput);
pub(super) fn scheduler<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<SchedulerInput>, D::Error> {
    Ok(Option::<SchedulerDecoded>::deserialize(d)?.map(|v| v.0))
}
pub(super) fn night<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<NightInput>, D::Error> {
    Ok(Option::<NightDecoded>::deserialize(d)?.map(|v| v.0))
}
fn accepted_budget<S: serde::Serializer>(
    v: &AcceptedOutputBudget,
    s: S,
) -> std::result::Result<S::Ok, S::Error> {
    match v {
        AcceptedOutputBudget::Accepted(v) => v.serialize(s),
        AcceptedOutputBudget::MissingLegacy => Err(serde::ser::Error::custom(
            "missing legacy cognition input authority",
        )),
    }
}

/// Allocation-free projection of an existing flight. MissingLegacy is never
/// serializable, including through preflight. No current actor lookup occurs.
#[derive(Serialize)]
pub(crate) struct SchedulerInputRef<'a> {
    pub method: CognitionRequestMethod,
    pub actor_id: &'a ActorId,
    pub presence_epoch: u64,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    pub request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    pub semantic: OperationId,
    pub lane: SchedulerInputLane,
    pub prompt: &'a str,
    #[serde(serialize_with = "accepted_budget")]
    pub output_token_budget: AcceptedOutputBudget,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NightSubjectRef<'a> {
    Person(&'a ActorId),
    Ward(crate::lore::PlanningWard),
}
#[derive(Serialize)]
pub(crate) struct NightInputRef<'a> {
    pub method: CognitionRequestMethod,
    pub subject: NightSubjectRef<'a>,
    pub presence_epoch: Option<u64>,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    pub request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    pub semantic: OperationId,
    pub owed_day: i64,
    pub prompt: &'a str,
    #[serde(serialize_with = "accepted_budget")]
    pub output_token_budget: AcceptedOutputBudget,
}
fn budget(v: AcceptedOutputBudget) -> Result<Option<u32>> {
    match v {
        AcceptedOutputBudget::Accepted(v) => Ok(v),
        AcceptedOutputBudget::MissingLegacy => Err(CheckpointError::new(
            OWNER,
            "missing legacy cognition input authority",
        )),
    }
}
fn budget_agrees(saved: Option<u32>, live: AcceptedOutputBudget) -> bool {
    match live {
        AcceptedOutputBudget::Accepted(v) => saved == v,
        // A separately supplied, mandatory sidecar is the saved authority.
        // The legacy owner alone cannot provide or reconstruct this scalar.
        AcceptedOutputBudget::MissingLegacy => true,
    }
}
impl SchedulerInputRef<'_> {
    pub(super) fn copy(&self) -> Result<SchedulerInput> {
        let output_token_budget = budget(self.output_token_budget)?;
        Ok(SchedulerInput {
            method: self.method,
            actor_id: self.actor_id.clone(),
            presence_epoch: self.presence_epoch,
            request_id: self.request_id,
            semantic: self.semantic,
            lane: self.lane,
            prompt: self.prompt.into(),
            output_token_budget,
        })
    }
    pub(super) fn agrees(&self, v: &SchedulerInput) -> bool {
        self.method == v.method
            && self.actor_id == &v.actor_id
            && self.presence_epoch == v.presence_epoch
            && self.request_id == v.request_id
            && self.semantic == v.semantic
            && self.lane == v.lane
            && self.prompt == v.prompt
            && budget_agrees(v.output_token_budget, self.output_token_budget)
    }
}
impl NightInputRef<'_> {
    pub(super) fn copy(&self) -> Result<NightInput> {
        let output_token_budget = budget(self.output_token_budget)?;
        let subject = match self.subject {
            NightSubjectRef::Person(a) => NightInputSubject::Person(a.clone()),
            NightSubjectRef::Ward(w) => NightInputSubject::Ward(w),
        };
        Ok(NightInput {
            method: self.method,
            subject,
            presence_epoch: self.presence_epoch,
            request_id: self.request_id,
            semantic: self.semantic,
            owed_day: self.owed_day,
            prompt: self.prompt.into(),
            output_token_budget,
        })
    }
    pub(super) fn agrees(&self, v: &NightInput) -> bool {
        let subject = match (&self.subject, &v.subject) {
            (NightSubjectRef::Person(a), NightInputSubject::Person(b)) => *a == b,
            (NightSubjectRef::Ward(a), NightInputSubject::Ward(b)) => a == b,
            _ => false,
        };
        subject
            && self.method == v.method
            && self.presence_epoch == v.presence_epoch
            && self.request_id == v.request_id
            && self.semantic == v.semantic
            && self.owed_day == v.owed_day
            && self.prompt == v.prompt
            && budget_agrees(v.output_token_budget, self.output_token_budget)
    }
}
impl SchedulerInput {
    pub fn method(&self) -> CognitionRequestMethod {
        self.method
    }
    pub fn actor_id(&self) -> &ActorId {
        &self.actor_id
    }
    pub fn presence_epoch(&self) -> u64 {
        self.presence_epoch
    }
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }
    pub fn semantic(&self) -> OperationId {
        self.semantic
    }
    pub fn lane(&self) -> SchedulerInputLane {
        self.lane
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn output_token_budget(&self) -> Option<u32> {
        self.output_token_budget
    }
}
impl NightInput {
    pub fn method(&self) -> CognitionRequestMethod {
        self.method
    }
    pub fn subject(&self) -> &NightInputSubject {
        &self.subject
    }
    pub fn presence_epoch(&self) -> Option<u64> {
        self.presence_epoch
    }
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }
    pub fn semantic(&self) -> OperationId {
        self.semantic
    }
    pub fn owed_day(&self) -> i64 {
        self.owed_day
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn output_token_budget(&self) -> Option<u32> {
        self.output_token_budget
    }
}
