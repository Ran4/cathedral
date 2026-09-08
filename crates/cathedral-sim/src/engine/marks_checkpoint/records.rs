//! Closed intrinsic ChalkStanding cache shape.
use super::*;
use crate::checkpoint::{records as common, serde_support::remote_adapters};
#[derive(Serialize, Deserialize)]
#[serde(remote = "ChalkableHere", deny_unknown_fields)]
pub(crate) struct ChalkableV1 {
    #[serde(with = "common::TextV1")]
    handle: String,
    #[serde(with = "label")]
    label: String,
    kinds: Vec<crate::marks::MarkKind>,
}
remote_adapters!(chalkable, ChalkableHere, ChalkableV1);
pub(crate) struct ChalkStandingV1;
impl ChalkStandingV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &EngineMessage,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            pen: bool,
            #[serde(with = "chalkable::vec")]
            anchors: &'a [ChalkableHere],
        }
        match v {
            EngineMessage::ChalkStanding { pen, anchors } => {
                Ref { pen: *pen, anchors }.serialize(s)
            }
            _ => Err(serde::ser::Error::custom("invalid chalk cache variant")),
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<EngineMessage, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            pen: bool,
            #[serde(with = "chalkable::vec")]
            anchors: Vec<ChalkableHere>,
        }
        let w = Wire::deserialize(d)?;
        Ok(EngineMessage::ChalkStanding {
            pen: w.pen,
            anchors: w.anchors,
        })
    }
}
remote_adapters!(standing, EngineMessage, ChalkStandingV1);
mod label {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &str, s: S) -> std::result::Result<S::Ok, S::Error> {
        if v.len() > MAX_LABEL_BYTES {
            return Err(serde::ser::Error::custom("chalk label byte limit"));
        }
        s.serialize_str(v)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<String, D::Error> {
        struct Text;
        impl serde::de::Visitor<'_> for Text {
            type Value = String;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded chalk label")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<String, E> {
                if v.len() > MAX_LABEL_BYTES {
                    return Err(E::custom("chalk label byte limit"));
                }
                Ok(v.to_owned())
            }
        }
        d.deserialize_str(Text)
    }
}
