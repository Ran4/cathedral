//! Closed records reconstruct every private field directly.
#![allow(private_interfaces)]
use super::*;
use crate::checkpoint::{
    records::{OperationIdV1, TextV1},
    serde_support::{remote_adapters, required_option, unique_map},
};
#[derive(Serialize, Deserialize)]
#[serde(remote = "NightOfficeConfig", deny_unknown_fields)]
pub(crate) struct ConfigV1 {
    enabled: bool,
    majors: bool,
    wards: bool,
    ambients: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Subject", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SubjectV1 {
    Person(ActorId),
    Ward(PlanningWard),
}
remote_adapters!(operation, OperationId, OperationIdV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Due", deny_unknown_fields)]
struct DueV1 {
    #[serde(skip)]
    queued_presence_epoch: Option<u64>,
    #[serde(with = "operation::option")]
    semantic: Option<OperationId>,
    #[serde(deserialize_with = "required_option")]
    presence_epoch: Option<u64>,
    #[serde(with = "SubjectV1")]
    subject: Subject,
    day: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Flight", deny_unknown_fields)]
struct FlightV1 {
    // V1 predates resolved budget authority. Never turn its absence into None.
    #[serde(skip)]
    output_token_budget: crate::traits::AcceptedOutputBudget,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    owed_day: i64,
    #[serde(deserialize_with = "required_option")]
    presence_epoch: Option<u64>,
    #[serde(with = "SubjectV1")]
    subject: Subject,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "TextV1")]
    prompt: String,
}
remote_adapters!(flight, Flight, FlightV1);
mod queue {
    use super::*;
    #[derive(Serialize, Deserialize)]
    struct Row(#[serde(with = "DueV1")] Due);
    pub fn serialize<S: serde::Serializer>(
        v: &VecDeque<Due>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        #[derive(Serialize)]
        struct Ref<'a>(#[serde(with = "DueV1")] &'a Due);
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for row in v {
            seq.serialize_element(&Ref(row))?;
        }
        seq.end()
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<VecDeque<Due>, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = VecDeque<Due>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("ordered Night duties")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = VecDeque::new();
                while let Some(Row(row)) = a.next_element()? {
                    out.push_back(row);
                }
                Ok(out)
            }
        }
        d.deserialize_seq(V)
    }
}
mod stamps {
    use super::*;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row {
        #[serde(with = "SubjectV1")]
        subject: Subject,
        day: i64,
    }
    pub fn serialize<S: serde::Serializer>(
        v: &BTreeMap<Subject, i64>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        #[derive(Serialize)]
        struct Ref<'a> {
            #[serde(with = "SubjectV1")]
            subject: &'a Subject,
            day: i64,
        }
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for (subject, day) in v {
            seq.serialize_element(&Ref { subject, day: *day })?;
        }
        seq.end()
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<BTreeMap<Subject, i64>, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BTreeMap<Subject, i64>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("unique Night daily stamps")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = BTreeMap::new();
                while let Some(Row { subject, day }) = a.next_element()? {
                    if out.insert(subject, day).is_some() {
                        return Err(serde::de::Error::custom("duplicate Night stamp"));
                    }
                }
                Ok(out)
            }
        }
        d.deserialize_seq(V)
    }
}
/// +Infinity is a reachable future that no finite supported poll can reach.
/// Historical pacing can remain after a scale change; do not recompute it.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Future {
    Never,
    At(f64),
}
mod future {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        if *v == f64::INFINITY {
            Future::Never.serialize(s)
        } else if v.is_finite() && *v >= 0.0 {
            Future::At(*v).serialize(s)
        } else {
            Err(serde::ser::Error::custom("invalid Night future anchor"))
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        match Future::deserialize(d)? {
            Future::Never => Ok(f64::INFINITY),
            Future::At(x) if x.is_finite() && x >= 0.0 => Ok(x),
            _ => Err(serde::de::Error::custom("invalid Night future anchor")),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "NightOffice", deny_unknown_fields)]
pub(crate) struct NightV1 {
    #[serde(with = "ConfigV1")]
    config: NightOfficeConfig,
    #[serde(with = "queue")]
    queue: VecDeque<Due>,
    #[serde(with = "flight::option")]
    in_flight: Option<Flight>,
    #[serde(skip)]
    load_retry_pending: bool,
    #[serde(with = "crate::traits::checkpoint::completion::option")]
    held_result: Option<Completion>,
    #[serde(with = "stamps")]
    last_reflected: BTreeMap<Subject, i64>,
    #[serde(with = "unique_map")]
    bedtimes: BTreeMap<ActorId, Office>,
    last_office_days: f64,
    #[serde(deserialize_with = "required_option")]
    last_ambient_reroll_day: Option<i64>,
    #[serde(with = "future")]
    next_attempt_at: f64,
    next_yield_report: f64,
    seeded: bool,
    reflected: u64,
    dropped: u64,
}
