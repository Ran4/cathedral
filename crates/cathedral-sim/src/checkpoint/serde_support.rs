//! Strict collection adapters for explicit owner DTO definitions. Aggregate
//! admission has already run before these collections can allocate.
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
};

pub(crate) mod unique_map {
    use super::*;
    pub fn serialize<S: Serializer, K: Serialize, V: Serialize>(
        value: &BTreeMap<K, V>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(s)
    }
    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    >(
        d: D,
    ) -> Result<BTreeMap<K, V>, D::Error> {
        struct Map<K, V>(PhantomData<(K, V)>);
        impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> Visitor<'de> for Map<K, V> {
            type Value = BTreeMap<K, V>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map with unique keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some(k) = a.next_key()? {
                    if map.contains_key(&k) {
                        return Err(de::Error::custom("duplicate map key"));
                    }
                    map.insert(k, a.next_value()?);
                }
                Ok(map)
            }
        }
        d.deserialize_map(Map(PhantomData))
    }
}
pub(crate) mod unique_set {
    use super::*;
    pub fn serialize<S: Serializer, T: Serialize>(
        value: &BTreeSet<T>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de> + Ord>(
        d: D,
    ) -> Result<BTreeSet<T>, D::Error> {
        struct Set<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de> + Ord> Visitor<'de> for Set<T> {
            type Value = BTreeSet<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a set with unique entries")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut set = BTreeSet::new();
                while let Some(v) = a.next_element()? {
                    if !set.insert(v) {
                        return Err(de::Error::custom("duplicate set entry"));
                    }
                }
                Ok(set)
            }
        }
        d.deserialize_seq(Set(PhantomData))
    }
}

/// `deserialize_with` makes an Option field required, while still admitting an
/// explicit null. Plain derived Option would silently default a missing field.
pub(crate) fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

macro_rules! remote_adapters {
    ($module:ident, $ty:ty, $remote:path) => {
        #[allow(dead_code)]
        pub(crate) mod $module {
            use super::*;
            struct Ref<'a>(&'a $ty);
            impl serde::Serialize for Ref<'_> {
                fn serialize<S: serde::Serializer>(
                    &self,
                    s: S,
                ) -> std::result::Result<S::Ok, S::Error> {
                    <$remote>::serialize(self.0, s)
                }
            }
            struct Owned($ty);
            impl<'de> serde::Deserialize<'de> for Owned {
                fn deserialize<D: serde::Deserializer<'de>>(
                    d: D,
                ) -> std::result::Result<Self, D::Error> {
                    <$remote>::deserialize(d).map(Self)
                }
            }
            pub(crate) mod option {
                use super::*;
                pub fn serialize<S: serde::Serializer>(
                    v: &Option<$ty>,
                    s: S,
                ) -> std::result::Result<S::Ok, S::Error> {
                    v.as_ref().map(Ref).serialize(s)
                }
                pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                    d: D,
                ) -> std::result::Result<Option<$ty>, D::Error> {
                    Option::<Owned>::deserialize(d).map(|v| v.map(|x| x.0))
                }
            }
            pub(crate) mod vec {
                use super::*;
                pub fn serialize<S: serde::Serializer>(
                    v: &[$ty],
                    s: S,
                ) -> std::result::Result<S::Ok, S::Error> {
                    use serde::ser::SerializeSeq;
                    let mut seq = s.serialize_seq(Some(v.len()))?;
                    for x in v {
                        seq.serialize_element(&Ref(x))?;
                    }
                    seq.end()
                }
                pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                    d: D,
                ) -> std::result::Result<Vec<$ty>, D::Error> {
                    struct V;
                    impl<'de> serde::de::Visitor<'de> for V {
                        type Value = Vec<$ty>;
                        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                            f.write_str("owner records")
                        }
                        fn visit_seq<A: serde::de::SeqAccess<'de>>(
                            self,
                            mut a: A,
                        ) -> std::result::Result<Self::Value, A::Error> {
                            let mut out = Vec::new();
                            while let Some(Owned(v)) = a.next_element()? {
                                out.push(v);
                            }
                            Ok(out)
                        }
                    }
                    d.deserialize_seq(V)
                }
            }
            pub(crate) mod map {
                use super::*;
                pub fn serialize<S: serde::Serializer, K: serde::Serialize>(
                    v: &std::collections::BTreeMap<K, $ty>,
                    s: S,
                ) -> std::result::Result<S::Ok, S::Error> {
                    use serde::ser::SerializeMap;
                    let mut map = s.serialize_map(Some(v.len()))?;
                    for (k, x) in v {
                        map.serialize_entry(k, &Ref(x))?;
                    }
                    map.end()
                }
                pub fn deserialize<
                    'de,
                    D: serde::Deserializer<'de>,
                    K: serde::Deserialize<'de> + Ord,
                >(
                    d: D,
                ) -> std::result::Result<std::collections::BTreeMap<K, $ty>, D::Error> {
                    struct V<K>(std::marker::PhantomData<K>);
                    impl<'de, K: serde::Deserialize<'de> + Ord> serde::de::Visitor<'de> for V<K> {
                        type Value = std::collections::BTreeMap<K, $ty>;
                        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                            f.write_str("uniquely keyed owner records")
                        }
                        fn visit_map<A: serde::de::MapAccess<'de>>(
                            self,
                            mut a: A,
                        ) -> std::result::Result<Self::Value, A::Error> {
                            let mut out = std::collections::BTreeMap::new();
                            while let Some(k) = a.next_key()? {
                                if out.contains_key(&k) {
                                    return Err(serde::de::Error::custom("duplicate owner key"));
                                }
                                let Owned(v) = a.next_value()?;
                                out.insert(k, v);
                            }
                            Ok(out)
                        }
                    }
                    d.deserialize_map(V(std::marker::PhantomData))
                }
            }
        }
    };
}
pub(crate) use remote_adapters;
