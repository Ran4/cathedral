#![allow(private_interfaces)]
use super::*;
use crate::checkpoint::serde_support::{remote_adapters, required_option};
pub(crate) mod pairs {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row<A> {
        a: A,
        b: A,
        at: f64,
    }
    pub fn serialize<S: serde::Serializer>(
        v: &BTreeMap<(ActorId, ActorId), f64>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|((a, b), at)| Row { a, b, at: *at }))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<BTreeMap<(ActorId, ActorId), f64>, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = BTreeMap<(ActorId, ActorId), f64>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("distinct canonical warm pairs")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = BTreeMap::new();
                while let Some(r) = a.next_element::<Row<ActorId>>()? {
                    if out.len() >= social::MAX_WARM_PAIRS
                        || r.a >= r.b
                        || out.insert((r.a, r.b), r.at).is_some()
                    {
                        return Err(serde::de::Error::custom(
                            "duplicate, noncanonical or excessive warm pair",
                        ));
                    }
                }
                Ok(out)
            }
        }
        d.deserialize_seq(Visitor)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WarmExchanges", deny_unknown_fields)]
pub(crate) struct WarmExchangesV1 {
    #[serde(with = "pairs")]
    pairs: BTreeMap<(ActorId, ActorId), f64>,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Memory", deny_unknown_fields)]
pub(crate) struct MemoryV1 {
    #[serde(deserialize_with = "required_option")]
    context: Option<u64>,
    visit: u64,
    touched_at: f64,
}
remote_adapters!(memory, Memory, MemoryV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Novelty", deny_unknown_fields)]
pub(crate) struct NoveltyV1 {
    #[serde(with = "memory::map")]
    last_told: BTreeMap<ActorId, Memory>,
}
