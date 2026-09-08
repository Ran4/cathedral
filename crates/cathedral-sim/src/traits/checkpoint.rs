//! Exact closed backend-value records; no service restoration or failure helper.
use super::*;
use crate::checkpoint::{records::TextV1, serde_support::remote_adapters};
#[derive(Serialize, Deserialize)]
#[serde(remote = "CognitionError", deny_unknown_fields)]
pub(crate) struct CognitionErrorV1 {
    #[serde(with = "TextV1")]
    kind: String,
    #[serde(with = "TextV1")]
    detail: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ResultWire {
    Ok(crate::checkpoint::BoundedText<{ crate::MAX_LLM_REPLY_CHARS * 4 }>),
    Err(#[serde(with = "CognitionErrorV1")] CognitionError),
}
mod result {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &Result<String, CognitionError>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        enum View<'a> {
            Ok(&'a str),
            Err(#[serde(with = "CognitionErrorV1")] &'a CognitionError),
        }
        match v {
            Ok(t) => View::Ok(t).serialize(s),
            Err(e) => View::Err(e).serialize(s),
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Result<String, CognitionError>, D::Error> {
        ResultWire::deserialize(d).map(|v| match v {
            ResultWire::Ok(t) => Ok(t.0),
            ResultWire::Err(e) => Err(e),
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Completion", deny_unknown_fields)]
pub(crate) struct CompletionV1 {
    #[serde(with = "request_id")]
    request_id: RequestId,
    #[serde(with = "result")]
    result: Result<String, CognitionError>,
    duration_seconds: f64,
}
remote_adapters!(completion, Completion, CompletionV1);

pub(crate) mod request_id {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &RequestId, s: S) -> Result<S::Ok, S::Error> {
        v.0.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<RequestId, D::Error> {
        u64::deserialize(d).map(RequestId)
    }
}
