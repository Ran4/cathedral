use super::*;
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "IdleCognitionMode",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum IdleModeV1 {
    All,
    Stage,
}
mod raw_float {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Bits {
        bits: u64,
    }
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        Bits { bits: v.to_bits() }.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        Ok(f64::from_bits(Bits::deserialize(d)?.bits))
    }
}
mod count {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &usize,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        u64::try_from(*v)
            .map_err(serde::ser::Error::custom)?
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<usize, D::Error> {
        usize::try_from(u64::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "StageConfig", deny_unknown_fields)]
pub(super) struct StageV1 {
    #[serde(with = "raw_float")]
    radius_m: f64,
    #[serde(with = "count")]
    max_actors: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "CuriosityConfig", deny_unknown_fields)]
pub(super) struct CuriosityV1 {
    enabled: bool,
    #[serde(with = "raw_float")]
    scale: f64,
}
