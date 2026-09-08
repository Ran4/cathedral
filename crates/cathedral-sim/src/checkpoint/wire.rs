use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::{fmt, marker::PhantomData};

/// Bound collection growth while decoding, before accepting the extra value.
/// Do not trust size_hint from an untrusted deserializer for preallocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct BoundedVec<T, const N: usize>(pub(crate) Vec<T>);
impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for BoundedVec<T, N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Values<T, const N: usize>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Values<T, N> {
            type Value = BoundedVec<T, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "at most {N} records")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut rows = Vec::new();
                while rows.len() < N {
                    let Some(row) = a.next_element()? else {
                        return Ok(BoundedVec(rows));
                    };
                    // Geometric growth capped at N, not a realloc per row or
                    // an unchecked allocator hint from the wire.
                    if rows.len() == rows.capacity() {
                        let next = rows.capacity().saturating_mul(2).max(8).min(N);
                        rows.reserve_exact(next - rows.len());
                    }
                    rows.push(row);
                }
                // This visitor fails immediately on any extra element, without
                // constructing an unbounded value just to report count overflow.
                if a.next_element::<RejectExtra>()?.is_some() {
                    unreachable!();
                }
                Ok(BoundedVec(rows))
            }
        }
        d.deserialize_seq(Values::<T, N>(PhantomData))
    }
}
struct RejectExtra;
impl<'de> Deserialize<'de> for RejectExtra {
    fn deserialize<D: Deserializer<'de>>(_: D) -> std::result::Result<Self, D::Error> {
        Err(de::Error::custom("record count limit exceeded"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct BoundedText<const N: usize>(pub(crate) String);
impl<const N: usize> BoundedText<N> {
    pub(crate) fn new(value: &str) -> std::result::Result<Self, &'static str> {
        if value.len() > N {
            return Err("string byte limit exceeded");
        }
        Ok(Self(value.to_owned()))
    }
}
impl<'de, const N: usize> Deserialize<'de> for BoundedText<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Text<const N: usize>;
        impl<const N: usize> Visitor<'_> for Text<N> {
            type Value = BoundedText<N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a string of at most {N} UTF-8 bytes")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> std::result::Result<Self::Value, E> {
                BoundedText::new(s).map_err(E::custom)
            }
        }
        d.deserialize_str(Text::<N>)
    }
}

/// Counting serializer sink; never allocates the encoded record. The caller
/// must validate floats first because serde_json otherwise encodes NaN as null.
pub(crate) fn encoded_len<T: Serialize>(
    value: &T,
    limit: usize,
) -> std::result::Result<usize, String> {
    struct Count {
        bytes: usize,
        limit: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self
                .bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= self.limit)
                .ok_or_else(|| std::io::Error::other("encoded byte limit exceeded"))?;
            self.bytes = next;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count { bytes: 0, limit };
    serde_json::to_writer(&mut count, value).map_err(|e| e.to_string())?;
    Ok(count.bytes)
}
