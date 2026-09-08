//! Closed semantic input records. No execution state is deserialized as a router.
use super::*;
use crate::checkpoint::{
    records::{CommandIdV1, command},
    serde_support::{remote_adapters, required_option},
};
use crate::receipts::{AffectedRef, Outcome, Receipt, ReceiptState};
#[derive(Serialize, Deserialize)]
#[serde(remote = "Outcome", deny_unknown_fields)]
struct OutcomeV1 {
    #[serde(deserialize_with = "receipt_state")]
    state: ReceiptState,
    code: String,
    message: String,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "AffectedRef", deny_unknown_fields)]
struct AffectedV1 {
    kind: String,
    id: String,
}
remote_adapters!(affected, AffectedRef, AffectedV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Receipt", deny_unknown_fields)]
pub(crate) struct ReceiptV1 {
    #[serde(with = "CommandIdV1")]
    id: CommandId,
    ordinal: u64,
    at: f64,
    #[serde(with = "OutcomeV1")]
    outcome: Outcome,
    #[serde(with = "affected::vec")]
    affected: Vec<AffectedRef>,
}
remote_adapters!(receipt, Receipt, ReceiptV1);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bits {
    bits: u64,
}
pub(crate) mod optional_bits {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &Option<f64>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.map(|x| Bits { bits: x.to_bits() }).serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Option<f64>, D::Error> {
        Option::<Bits>::deserialize(d).map(|v| v.map(|x| f64::from_bits(x.bits)))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPurpose {
    PublicPlayerSpeech,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionStatus {
    InterruptedUnsent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingSource {
    BatchPending,
    Parked,
}
#[derive(Debug, Serialize)]
pub struct InterruptedStream {
    pub(super) basename: String,
    #[serde(deserialize_with = "required_option")]
    pub(super) available_text: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "InterruptedStream", deny_unknown_fields)]
struct InterruptedStreamV1 {
    basename: String,
    #[serde(deserialize_with = "required_option")]
    available_text: Option<String>,
}
remote_adapters!(stream, InterruptedStream, InterruptedStreamV1);
impl InterruptedStream {
    pub fn basename(&self) -> &str {
        &self.basename
    }
    pub fn available_text(&self) -> Option<&str> {
        self.available_text.as_deref()
    }
}
#[derive(Debug, Serialize)]
pub struct AcceptedRecording {
    pub(super) source: RecordingSource,
    #[serde(with = "command::option")]
    pub(super) semantic: Option<CommandId>,
    #[serde(with = "receipt::option")]
    pub(super) receipt: Option<Receipt>,
    pub(super) request_id: String,
    pub(super) basename: String,
    #[serde(with = "crate::math::vec3_serde")]
    pub(super) position_m: Vec3,
    #[serde(deserialize_with = "backend")]
    pub(super) backend: SttBackendKind,
    #[serde(with = "optional_bits")]
    pub(super) parked_deadline: Option<f64>,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "AcceptedRecording", deny_unknown_fields)]
struct AcceptedRecordingV1 {
    source: RecordingSource,
    #[serde(with = "command::option")]
    semantic: Option<CommandId>,
    #[serde(with = "receipt::option")]
    receipt: Option<Receipt>,
    request_id: String,
    basename: String,
    #[serde(with = "crate::math::vec3_serde")]
    position_m: Vec3,
    #[serde(deserialize_with = "backend")]
    backend: SttBackendKind,
    #[serde(with = "optional_bits")]
    parked_deadline: Option<f64>,
}
remote_adapters!(recording, AcceptedRecording, AcceptedRecordingV1);
impl AcceptedRecording {
    pub fn source(&self) -> RecordingSource {
        self.source
    }
    pub fn semantic(&self) -> Option<CommandId> {
        self.semantic
    }
    pub fn receipt(&self) -> Option<&Receipt> {
        self.receipt.as_ref()
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn basename(&self) -> &str {
        &self.basename
    }
    pub fn position_m(&self) -> Vec3 {
        self.position_m
    }
    pub fn backend(&self) -> SttBackendKind {
        self.backend
    }
    pub fn parked_deadline(&self) -> Option<f64> {
        self.parked_deadline
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateV1 {
    #[serde(with = "raw_float")]
    pub stt_stream_grace_seconds: f64,
    pub purpose: InputPurpose,
    pub status: InterruptionStatus,
    pub captures: Vec<String>,
    #[serde(with = "stream::vec")]
    pub streams: Vec<InterruptedStream>,
    #[serde(with = "recording::vec")]
    pub accepted_recordings: Vec<AcceptedRecording>,
}
/// Stack-only projections used by the serializer before any owned copy.
#[derive(Serialize)]
pub(super) struct StreamView<'a> {
    pub basename: &'a str,
    pub available_text: &'a Option<String>,
}
#[derive(Serialize)]
pub(super) struct RecordingView<'a> {
    pub source: RecordingSource,
    #[serde(with = "command::option")]
    pub semantic: Option<CommandId>,
    pub receipt: Option<crate::receipts::CheckpointReceiptRef<'a>>,
    pub request_id: &'a str,
    pub basename: &'a str,
    #[serde(with = "crate::math::vec3_serde")]
    pub position_m: Vec3,
    pub backend: SttBackendKind,
    #[serde(with = "optional_bits")]
    pub parked_deadline: Option<f64>,
}

macro_rules! closed_string_enum {
    ($ty:ident, $($text:literal => $value:path),+ $(,)?) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{
                struct V;
                impl serde::de::Visitor<'_> for V {
                    type Value=$ty;
                    fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("a canonical speech enum string")}
                    fn visit_str<E:serde::de::Error>(self,v:&str)->std::result::Result<$ty,E>{match v {$($text=>Ok($value),)+_=>Err(E::custom("unknown speech variant"))}}
                }
                d.deserialize_str(V)
            }
        }
    };
}
closed_string_enum!(InputPurpose,"public_player_speech"=>InputPurpose::PublicPlayerSpeech);
closed_string_enum!(InterruptionStatus,"interrupted_unsent"=>InterruptionStatus::InterruptedUnsent);
closed_string_enum!(RecordingSource,"batch_pending"=>RecordingSource::BatchPending,"parked"=>RecordingSource::Parked);

fn backend<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<SttBackendKind, D::Error> {
    struct V;
    impl serde::de::Visitor<'_> for V {
        type Value = SttBackendKind;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("cloud or local")
        }
        fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<Self::Value, E> {
            match s {
                "cloud" => Ok(SttBackendKind::Cloud),
                "local" => Ok(SttBackendKind::Local),
                _ => Err(E::custom("invalid STT backend")),
            }
        }
    }
    d.deserialize_str(V)
}
fn receipt_state<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<ReceiptState, D::Error> {
    struct V;
    impl serde::de::Visitor<'_> for V {
        type Value = ReceiptState;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a canonical receipt state string")
        }
        fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<Self::Value, E> {
            match s {
                "rejected" => Ok(ReceiptState::Rejected),
                "accepted" => Ok(ReceiptState::Accepted),
                "in_progress" => Ok(ReceiptState::InProgress),
                "completed" => Ok(ReceiptState::Completed),
                "interrupted" => Ok(ReceiptState::Interrupted),
                "superseded" => Ok(ReceiptState::Superseded),
                _ => Err(E::custom("invalid receipt state")),
            }
        }
    }
    d.deserialize_str(V)
}
