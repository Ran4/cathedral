use super::*;
use serde::de::{self, IntoDeserializer, Visitor};
use std::fmt;

// Serde's externally tagged unit variants also accept {"unit": null}.
// Host wire admits only its single documented string spelling.
macro_rules! mixed {
    ($ty:ty, $data:ty, {$($text:literal => $unit:expr),*}, $convert:expr) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{
                struct V;
                impl<'de> Visitor<'de> for V {
                    type Value=$ty;
                    fn expecting(&self,f:&mut fmt::Formatter)->fmt::Result{f.write_str("a closed host variant")}
                    fn visit_str<E:de::Error>(self,s:&str)->std::result::Result<Self::Value,E>{match s{$($text=>Ok($unit),)*_=>Err(E::custom("unknown host unit variant"))}}
                    fn visit_map<A:de::MapAccess<'de>>(self,a:A)->std::result::Result<Self::Value,A::Error>{<$data>::deserialize(de::value::MapAccessDeserializer::new(a)).map($convert)}
                }
                d.deserialize_any(V)
            }
        }
    };
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum BellData {
    NameKnell { years: u16 },
}
mixed!(BellV1,BellData,{"scold_curfew"=>BellV1::ScoldCurfew,"scold_summons"=>BellV1::ScoldSummons},|v|match v{BellData::NameKnell{years}=>BellV1::NameKnell{years}});
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ItemData {
    Pocketed(BodySlotV1),
}
mixed!(ItemSourceV1,ItemData,{"carried"=>ItemSourceV1::Carried},|v|match v{ItemData::Pocketed(v)=>ItemSourceV1::Pocketed(v)});

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PendingKindV1<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case", deny_unknown_fields)]
        enum Data<T> {
            Offer {
                item: T,
                target: T,
                quantity: Nullable<u32>,
            },
            Accept {
                item: T,
            },
            Decline {
                item: T,
            },
            Retract {
                item: T,
            },
            BodySlot {
                item: T,
            },
        }
        struct V<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = PendingKindV1<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a closed pending intent")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> std::result::Result<Self::Value, E> {
                Ok(match s {
                    "recording" => PendingKindV1::Recording,
                    "expel" => PendingKindV1::Expel,
                    "debug_say" => PendingKindV1::DebugSay,
                    "say" => PendingKindV1::Say,
                    _ => return Err(E::custom("unknown pending intent")),
                })
            }
            fn visit_map<A: de::MapAccess<'de>>(
                self,
                a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                Ok(
                    match Data::<T>::deserialize(de::value::MapAccessDeserializer::new(a))? {
                        Data::Offer {
                            item,
                            target,
                            quantity,
                        } => PendingKindV1::Offer {
                            item,
                            target,
                            quantity,
                        },
                        Data::Accept { item } => PendingKindV1::Accept { item },
                        Data::Decline { item } => PendingKindV1::Decline { item },
                        Data::Retract { item } => PendingKindV1::Retract { item },
                        Data::BodySlot { item } => PendingKindV1::BodySlot { item },
                    },
                )
            }
        }
        d.deserialize_any(V(std::marker::PhantomData))
    }
}

pub(super) mod logical_anchor {
    use super::*;
    use crate::checkpoint::LogicalAnchorV1;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case", deny_unknown_fields)]
    enum Data {
        At(crate::timeline::LogicalTime),
    }
    struct Closed(LogicalAnchorV1);
    mixed!(Closed,Data,{"never"=>Closed(LogicalAnchorV1::Never)},|v|match v{Data::At(v)=>Closed(LogicalAnchorV1::At(v))});
    pub fn serialize<S: serde::Serializer>(
        v: &LogicalAnchorV1,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<LogicalAnchorV1, D::Error> {
        Closed::deserialize(d).map(|v| v.0)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = Nullable<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("explicit null or a closed value")
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(Nullable(None))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(|v| Nullable(Some(v)))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                T::deserialize(de::value::SeqAccessDeserializer::new(a)).map(|v| Nullable(Some(v)))
            }
            fn visit_map<A: de::MapAccess<'de>>(
                self,
                a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                T::deserialize(de::value::MapAccessDeserializer::new(a)).map(|v| Nullable(Some(v)))
            }
        }
        // MissingFieldDeserializer only permits deserialize_option; calling
        // deserialize_any makes omission an error while JSON null visits unit.
        d.deserialize_any(V(std::marker::PhantomData))
    }
}

pub(super) mod duration {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct D {
        seconds: u64,
        nanos: u32,
    }
    pub fn serialize<S: serde::Serializer>(
        d: &Duration,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        D {
            seconds: d.as_secs(),
            nanos: d.subsec_nanos(),
        }
        .serialize(s)
    }
    pub fn deserialize<'de, Ds: serde::Deserializer<'de>>(
        d: Ds,
    ) -> std::result::Result<Duration, Ds::Error> {
        let d = D::deserialize(d)?;
        if d.nanos >= 1_000_000_000 || d.seconds > super::super::super::MAX_LOGICAL_SECONDS as u64 {
            return Err(de::Error::custom("host duration range"));
        }
        Ok(Duration::new(d.seconds, d.nanos))
    }
}
pub(super) mod nullable_duration {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    struct D(#[serde(with = "duration")] Duration);
    pub fn serialize<S: serde::Serializer>(
        d: &Nullable<Duration>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        Nullable(d.0.map(D)).serialize(s)
    }
    pub fn deserialize<'de, Ds: serde::Deserializer<'de>>(
        d: Ds,
    ) -> std::result::Result<Nullable<Duration>, Ds::Error> {
        Nullable::<D>::deserialize(d).map(|v| Nullable(v.0.map(|v| v.0)))
    }
}

macro_rules! closed_unit {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug,Clone,Copy,PartialEq,Eq)] pub enum $name { $($variant),+ }
        impl Serialize for $name {fn serialize<S:serde::Serializer>(&self,s:S)->std::result::Result<S::Ok,S::Error>{s.serialize_str(match self{$(Self::$variant=>$text),+})}}
        impl<'de> Deserialize<'de> for $name {fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{let t=String::deserialize(d)?;match t.as_str(){$($text=>Ok(Self::$variant)),+, _=>Err(de::Error::custom(concat!("unknown ",stringify!($name))))}}}
    };
}
closed_unit!(OfficeV1{Watch=>"watch",Kindling=>"kindling",Dayspring=>"dayspring",HighWick=>"high_wick",Waning=>"waning",Lamplight=>"lamplight",Snuffing=>"snuffing"});
closed_unit!(WeekdayV1{Bellday=>"bellday",Second=>"second",Highmarket=>"highmarket",Fourth=>"fourth",Fifth=>"fifth",Lowmarket=>"lowmarket",Seventh=>"seventh"});
closed_unit!(MarkKindV1{ChalkCross=>"chalk_cross",WellTally=>"well_tally",WardSign=>"ward_sign"});
closed_unit!(BodySlotV1{Mouth=>"mouth",Butt=>"butt",Frontbutt=>"frontbutt"});
closed_unit!(RungV1{Hearsay=>"hearsay",Word=>"word",Summoned=>"summoned",Warranted=>"warranted"});
closed_unit!(HudSlot{Subtitle=>"subtitle",PlayerTranscript=>"player_transcript",OfferOutcome=>"offer_outcome",Transient=>"transient",Inventory=>"inventory",OfferCard=>"offer_card",LawStanding=>"law_standing",JournalStanding=>"journal_standing",FocusHint=>"focus_hint"});
closed_unit!(WorkKind{Baking=>"baking",EelSmoking=>"eel_smoking",GlassFurnace=>"glass_furnace",CulletSorting=>"cullet_sorting",Weaving=>"weaving"});
closed_unit!(WellKind{Ford=>"ford",Chain=>"chain",ThreeCurb=>"three_curb"});
macro_rules! conversion {
    ($host:ty,$sim:ty,$($variant:ident),+)=>{impl From<$sim> for $host {fn from(v:$sim)->Self{match v{$(<$sim>::$variant=>Self::$variant),+}}}impl From<$host> for $sim {fn from(v:$host)->Self{match v{$(<$host>::$variant=>Self::$variant),+}}}};
}
conversion!(
    OfficeV1,
    crate::Office,
    Watch,
    Kindling,
    Dayspring,
    HighWick,
    Waning,
    Lamplight,
    Snuffing
);
conversion!(
    WeekdayV1,
    crate::Weekday,
    Bellday,
    Second,
    Highmarket,
    Fourth,
    Fifth,
    Lowmarket,
    Seventh
);
conversion!(
    MarkKindV1,
    crate::marks::MarkKind,
    ChalkCross,
    WellTally,
    WardSign
);
conversion!(BodySlotV1, crate::BodySlot, Mouth, Butt, Frontbutt);
conversion!(
    RungV1,
    crate::notices::Rung,
    Hearsay,
    Word,
    Summoned,
    Warranted
);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_wire_mixed_units_and_nullable_are_closed() {
        for text in [
            r#"{"scold_curfew":null}"#,
            r#"{"scold_summons":null}"#,
            r#"{"name_knell":{"years":3,"extra":0}}"#,
            r#"{"name_knell":{}}"#,
        ] {
            assert!(serde_json::from_str::<BellV1>(text).is_err(), "{text}");
        }
        assert!(serde_json::from_str::<ItemSourceV1>(r#"{"carried":null}"#).is_err());
        for unit in ["recording", "expel", "debug_say", "say"] {
            assert!(
                serde_json::from_str::<PendingKindV1<String>>(&format!(r#"{{"{unit}":null}}"#))
                    .is_err()
            );
            assert!(serde_json::from_str::<PendingKindV1<String>>(&format!("\"{unit}\"")).is_ok());
        }
        assert!(
            serde_json::from_str::<PendingKindV1<String>>(r#"{"offer":{"item":"i","target":"a"}}"#)
                .is_err()
        );
        #[derive(Deserialize)]
        struct Anchor {
            #[serde(with = "logical_anchor")]
            value: crate::checkpoint::LogicalAnchorV1,
        }
        assert!(serde_json::from_str::<Anchor>(r#"{"value":{"never":null}}"#).is_err());
        assert!(matches!(
            serde_json::from_str::<Anchor>(r#"{"value":"never"}"#)
                .unwrap()
                .value,
            crate::checkpoint::LogicalAnchorV1::Never
        ));
        assert!(
            std::mem::size_of::<RecordV1<String>>() + 64 < ROW_WORKING_BYTES,
            "inline row plus sort/index metadata charge"
        );
    }
}
