//! The single cached publication variant owned by existing law.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option},
};
#[derive(Serialize, Deserialize)]
#[serde(remote = "PlayerNotice", deny_unknown_fields)]
pub(crate) struct PlayerNoticeV1 {
    notice_id: u64,
    #[serde(with = "line")]
    line: String,
    #[serde(with = "owner::records::RungV1")]
    rung: notices::Rung,
    #[serde(with = "common::TextV1")]
    clears_when: String,
}
remote_adapters!(notice, PlayerNotice, PlayerNoticeV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "PlayerCustody", deny_unknown_fields)]
pub(crate) struct PlayerCustodyV1 {
    holder_ids: Vec<ActorId>,
    #[serde(deserialize_with = "required_option")]
    officer_id: Option<ActorId>,
    #[serde(with = "common::TextV1")]
    officer_name: String,
    #[serde(with = "common::TextV1")]
    station_name: String,
    #[serde(with = "crate::math::vec3_serde")]
    anchor_m: Vec3,
    leash_m: f64,
    tether_m: f64,
    reach_m: f64,
    closing: bool,
    strain_seconds: f64,
    held: bool,
    committed: bool,
    fee_sparks: u32,
    #[serde(with = "common::text::option")]
    release_office: Option<String>,
    #[serde(with = "common::text::option")]
    booked_as: Option<String>,
}
remote_adapters!(custody, PlayerCustody, PlayerCustodyV1);
pub(crate) struct LawStandingV1;
impl LawStandingV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &EngineMessage,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            #[serde(with = "notice::vec")]
            notices: &'a [PlayerNotice],
            #[serde(with = "custody::option")]
            custody: &'a Option<PlayerCustody>,
        }
        match v {
            EngineMessage::LawStanding { notices, custody } => {
                Ref { notices, custody }.serialize(s)
            }
            _ => Err(serde::ser::Error::custom("invalid law cache variant")),
        }
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<EngineMessage, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(with = "notice::vec")]
            notices: Vec<PlayerNotice>,
            #[serde(with = "custody::option")]
            custody: Option<PlayerCustody>,
        }
        let w = Wire::deserialize(d)?;
        Ok(EngineMessage::LawStanding {
            notices: w.notices,
            custody: w.custody,
        })
    }
}
remote_adapters!(standing, EngineMessage, LawStandingV1);

mod line {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &str, s: S) -> std::result::Result<S::Ok, S::Error> {
        if v.len() > MAX_NOTICE_LINE_BYTES {
            return Err(serde::ser::Error::custom("law cached line byte limit"));
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
                f.write_str("a bounded law publication line")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<String, E> {
                if v.len() > MAX_NOTICE_LINE_BYTES {
                    return Err(E::custom("law cached line byte limit"));
                }
                Ok(v.to_owned())
            }
        }
        d.deserialize_str(Text)
    }
}
