//! Closed checkpoint allocation accounting. This is deliberately not public:
//! custom Deserialize implementations require the complete-layout audit.
//!
//! Collection size hints are zero. Before the consumer receives a new element
//! we charge its real return type (including all inactive enum/Option storage),
//! a sparse first BTree node and subsequent nodes, and Vec capacity/reallocation.
//! Strings and the original JSON escape scratch are independent allocations.
use super::super::{CheckpointError, Reservation, Result};
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    mem::size_of,
};

pub(crate) const MAX_EXPANSION: usize = 128 * 1024 * 1024;
/// Existing owner validators use sequential definition/reference scratch. This
/// also retains a second ledger index and the explicit PlaceRegistry indexes.
pub(crate) const VALIDATION_SCRATCH: usize = 64 * 1024 * 1024;

pub(crate) struct DecodeMeter<'a> {
    reservation: RefCell<&'a mut Reservation>,
    base: usize,
    diagnostic_scratch: Cell<usize>,
    expanded: Cell<usize>,
    category: Cell<Option<usize>>,
    categories: RefCell<[usize; 16]>,
}
impl<'a> DecodeMeter<'a> {
    pub(crate) fn new(r: &'a mut Reservation, raw_capacity: usize) -> Result<Self> {
        let base = raw_capacity
            .checked_mul(3)
            .and_then(|n| n.checked_add(VALIDATION_SCRATCH))
            .ok_or_else(|| CheckpointError::new("complete", "input allocation overflow"))?;
        if r.bytes() < base {
            r.resize(base)?;
        }
        Ok(Self {
            reservation: RefCell::new(r),
            base,
            diagnostic_scratch: Cell::new(0),
            expanded: Cell::new(0),
            category: Cell::new(None),
            categories: RefCell::new([0; 16]),
        })
    }
    pub(crate) fn begin_category(&self, category: super::CheckpointCategory) {
        self.category.set(Some(category as usize));
    }
    pub(crate) fn categories(&self) -> [usize; 16] {
        *self.categories.borrow()
    }
    pub(crate) fn expanded(&self) -> usize {
        self.expanded.get()
    }
    pub(crate) fn diagnostic_scratch(&self) -> usize {
        self.diagnostic_scratch.get()
    }
    /// Enum payload/type errors can originate inside serde's VariantAccess,
    /// before a wrapped visitor runs. Precharge their temporary diagnostics too.
    /// An encoded token bounds decoded text; Debug escaping uses at most six
    /// output bytes per input byte. 32x covers formatter growth (old + new), the
    /// retained serde error and our bounded final diagnostic concurrently.
    pub(crate) fn prepare_diagnostics(&self, bytes: &[u8]) -> Result<()> {
        let mut longest = 0usize;
        let mut start = None;
        let mut escaped = false;
        for (index, &byte) in bytes.iter().enumerate() {
            if let Some(begin) = start {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    longest = longest.max(index - begin);
                    start = None;
                }
            } else if byte == b'"' {
                start = Some(index + 1);
            }
        }
        if let Some(begin) = start {
            longest = longest.max(bytes.len() - begin);
        }
        let wanted = longest
            .checked_mul(32)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| CheckpointError::new("complete", "diagnostic allowance overflow"))?;
        if wanted > self.diagnostic_scratch.get() {
            let total = self
                .base
                .checked_add(wanted)
                .and_then(|n| n.checked_add(self.expanded.get()))
                .ok_or_else(|| CheckpointError::new("complete", "diagnostic allowance overflow"))?;
            let mut reservation = self.reservation.borrow_mut();
            if reservation.bytes() < total {
                reservation.resize(total)?;
            }
            self.diagnostic_scratch.set(wanted);
        }
        Ok(())
    }
    pub(crate) fn charge(&self, bytes: usize) -> Result<()> {
        let next = self
            .expanded
            .get()
            .checked_add(bytes)
            .filter(|n| *n <= MAX_EXPANSION)
            .ok_or_else(|| {
                CheckpointError::new("complete", "complete decoded expansion limit exceeded")
            })?;
        let wanted = self.base + self.diagnostic_scratch.get() + next;
        let mut r = self.reservation.borrow_mut();
        if r.bytes() < wanted {
            r.resize(wanted)?;
        }
        self.expanded.set(next);
        if let Some(category) = self.category.get() {
            self.categories.borrow_mut()[category] += bytes;
        }
        Ok(())
    }
    pub(crate) fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T> {
        self.prepare_diagnostics(bytes)?;
        self.charge(size_of::<T>() + 256)?;
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let value = T::deserialize(Metered {
            inner: &mut decoder,
            meter: self,
            map_key: false,
        })
        .map_err(super::diagnostic)?;
        decoder.end().map_err(super::diagnostic)?;
        Ok(value)
    }
    fn debit<E: de::Error>(&self, bytes: usize) -> std::result::Result<(), E> {
        self.charge(bytes).map_err(E::custom)
    }
}

struct Metered<'a, 'b, D> {
    inner: D,
    meter: &'a DecodeMeter<'b>,
    map_key: bool,
}
struct Seed<'a, 'b, T> {
    inner: T,
    meter: &'a DecodeMeter<'b>,
    debit: usize,
    map_key: bool,
}
impl<'de, T: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<'_, '_, T> {
    type Value = T::Value;
    fn deserialize<D: de::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        self.meter.debit::<D::Error>(self.debit)?;
        self.inner.deserialize(Metered {
            inner: d,
            meter: self.meter,
            map_key: self.map_key,
        })
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Inline,
    Dynamic,
    Text,
    Identifier,
}
struct Visit<'a, 'b, V> {
    inner: V,
    meter: &'a DecodeMeter<'b>,
    kind: Kind,
    map_key: bool,
}

impl<V> Visit<'_, '_, V> {
    fn text_debit<E: de::Error>(&self, value: &str) -> std::result::Result<(), E> {
        if matches!(self.kind, Kind::Identifier | Kind::Inline) && value.len() > 256 {
            return Err(E::custom(
                "complete identifier or scalar-type string limit exceeded",
            ));
        }
        if !matches!(self.kind, Kind::Identifier) {
            self.meter.debit::<E>(4 * value.len() + 64)?;
        }
        Ok(())
    }
}
impl<'de, V: Visitor<'de>> Visitor<'de> for Visit<'_, '_, V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(f)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
        self.inner.visit_bool(value)
    }
    fn visit_i8<E: de::Error>(self, value: i8) -> std::result::Result<Self::Value, E> {
        self.inner.visit_i8(value)
    }
    fn visit_i16<E: de::Error>(self, value: i16) -> std::result::Result<Self::Value, E> {
        self.inner.visit_i16(value)
    }
    fn visit_i32<E: de::Error>(self, value: i32) -> std::result::Result<Self::Value, E> {
        self.inner.visit_i32(value)
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
        self.inner.visit_i64(value)
    }
    fn visit_i128<E: de::Error>(self, value: i128) -> std::result::Result<Self::Value, E> {
        self.inner.visit_i128(value)
    }
    fn visit_u8<E: de::Error>(self, value: u8) -> std::result::Result<Self::Value, E> {
        self.inner.visit_u8(value)
    }
    fn visit_u16<E: de::Error>(self, value: u16) -> std::result::Result<Self::Value, E> {
        self.inner.visit_u16(value)
    }
    fn visit_u32<E: de::Error>(self, value: u32) -> std::result::Result<Self::Value, E> {
        self.inner.visit_u32(value)
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
        self.inner.visit_u64(value)
    }
    fn visit_u128<E: de::Error>(self, value: u128) -> std::result::Result<Self::Value, E> {
        self.inner.visit_u128(value)
    }
    fn visit_f32<E: de::Error>(self, value: f32) -> std::result::Result<Self::Value, E> {
        self.inner.visit_f32(value)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
        self.inner.visit_f64(value)
    }
    fn visit_char<E: de::Error>(self, value: char) -> std::result::Result<Self::Value, E> {
        self.inner.visit_char(value)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
        self.text_debit::<E>(value)?;
        self.inner.visit_str(value)
    }
    fn visit_borrowed_str<E: de::Error>(
        self,
        value: &'de str,
    ) -> std::result::Result<Self::Value, E> {
        self.text_debit::<E>(value)?;
        self.inner.visit_borrowed_str(value)
    }
    fn visit_string<E: de::Error>(self, _: String) -> std::result::Result<Self::Value, E> {
        Err(E::custom(
            "unsupported preallocated string in closed decoder",
        ))
    }
    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> std::result::Result<Self::Value, E> {
        self.meter.debit::<E>(4 * value.len() + 64)?;
        self.inner.visit_bytes(value)
    }
    fn visit_borrowed_bytes<E: de::Error>(
        self,
        value: &'de [u8],
    ) -> std::result::Result<Self::Value, E> {
        self.meter.debit::<E>(4 * value.len() + 64)?;
        self.inner.visit_borrowed_bytes(value)
    }
    fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
        self.inner.visit_none()
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
        self.inner.visit_unit()
    }
    fn visit_some<D: de::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        self.inner.visit_some(Metered {
            inner: d,
            meter: self.meter,
            map_key: self.map_key,
        })
    }
    fn visit_newtype_struct<D: de::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        self.inner.visit_newtype_struct(Metered {
            inner: d,
            meter: self.meter,
            map_key: self.map_key,
        })
    }
    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> std::result::Result<Self::Value, A::Error> {
        self.inner.visit_seq(Sequence {
            inner: a,
            meter: self.meter,
            count: 0,
            dynamic: !matches!(self.kind, Kind::Inline),
        })
    }
    fn visit_map<A: MapAccess<'de>>(self, a: A) -> std::result::Result<Self::Value, A::Error> {
        self.inner.visit_map(Map {
            inner: a,
            meter: self.meter,
            count: 0,
            dynamic: !matches!(self.kind, Kind::Inline),
        })
    }
    fn visit_enum<A: EnumAccess<'de>>(self, a: A) -> std::result::Result<Self::Value, A::Error> {
        self.inner.visit_enum(Enum {
            inner: a,
            meter: self.meter,
        })
    }
}
struct Sequence<'a, 'b, A> {
    inner: A,
    meter: &'a DecodeMeter<'b>,
    count: usize,
    dynamic: bool,
}
impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for Sequence<'_, '_, A> {
    type Error = A::Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> std::result::Result<Option<T::Value>, Self::Error> {
        let debit = if self.dynamic {
            (if self.count == 0 { 16 } else { 6 }) * size_of::<T::Value>() + 128
        } else {
            0
        };
        let value = self.inner.next_element_seed(Seed {
            inner: seed,
            meter: self.meter,
            debit,
            map_key: false,
        })?;
        self.count += usize::from(value.is_some());
        Ok(value)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(0)
    }
}
struct Map<'a, 'b, A> {
    inner: A,
    meter: &'a DecodeMeter<'b>,
    count: usize,
    dynamic: bool,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for Map<'_, '_, A> {
    type Error = A::Error;
    fn next_key_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> std::result::Result<Option<T::Value>, Self::Error> {
        let debit = if self.dynamic {
            (if self.count == 0 { 11 } else { 8 }) * size_of::<T::Value>() + 128
        } else {
            0
        };
        self.inner.next_key_seed(Seed {
            inner: seed,
            meter: self.meter,
            debit,
            map_key: true,
        })
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> std::result::Result<T::Value, Self::Error> {
        let debit = if self.dynamic {
            (if self.count == 0 { 11 } else { 8 }) * size_of::<T::Value>()
        } else {
            0
        };
        self.count += 1;
        self.inner.next_value_seed(Seed {
            inner: seed,
            meter: self.meter,
            debit,
            map_key: false,
        })
    }
    fn size_hint(&self) -> Option<usize> {
        Some(0)
    }
}
struct Enum<'a, 'b, A> {
    inner: A,
    meter: &'a DecodeMeter<'b>,
}
impl<'de, 'a, 'b, A: EnumAccess<'de>> EnumAccess<'de> for Enum<'a, 'b, A> {
    type Error = A::Error;
    type Variant = Variant<'a, 'b, A::Variant>;
    fn variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> std::result::Result<(T::Value, Self::Variant), Self::Error> {
        let (tag, variant) = self.inner.variant_seed(Seed {
            inner: seed,
            meter: self.meter,
            debit: 0,
            map_key: false,
        })?;
        Ok((
            tag,
            Variant {
                inner: variant,
                meter: self.meter,
            },
        ))
    }
}
struct Variant<'a, 'b, A> {
    inner: A,
    meter: &'a DecodeMeter<'b>,
}
impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for Variant<'_, '_, A> {
    type Error = A::Error;
    fn unit_variant(self) -> std::result::Result<(), Self::Error> {
        self.inner.unit_variant()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> std::result::Result<T::Value, Self::Error> {
        self.inner.newtype_variant_seed(Seed {
            inner: seed,
            meter: self.meter,
            debit: 0,
            map_key: false,
        })
    }
    fn tuple_variant<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.tuple_variant(
            len,
            Visit {
                inner: visitor,
                meter: self.meter,
                kind: Kind::Inline,
                map_key: false,
            },
        )
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.struct_variant(
            fields,
            Visit {
                inner: visitor,
                meter: self.meter,
                kind: Kind::Inline,
                map_key: false,
            },
        )
    }
}
impl<'de, D: de::Deserializer<'de>> de::Deserializer<'de> for Metered<'_, '_, D> {
    type Error = D::Error;
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_bool<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_bool(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_i8<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_i8(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_i16<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_i16(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_i32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_i32(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_i64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_i64(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_i128<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_i128(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_u8<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_u8(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_u16<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_u16(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_u32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_u32(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_u64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_u64(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_u128<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_u128(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_f32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_f32(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_f64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_f64(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_char<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        let visitor = Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        };
        if self.map_key {
            self.inner.deserialize_char(visitor)
        } else {
            self.inner.deserialize_any(visitor)
        }
    }
    fn deserialize_str<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_str(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Text,
            map_key: self.map_key,
        })
    }
    fn deserialize_string<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_string(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Text,
            map_key: self.map_key,
        })
    }
    fn deserialize_bytes<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_bytes(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_byte_buf(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_option<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_option(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_seq<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_map<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_identifier<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_identifier(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Identifier,
            map_key: self.map_key,
        })
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_ignored_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Dynamic,
            map_key: self.map_key,
        })
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_newtype_struct(
            name,
            Visit {
                inner: visitor,
                meter: self.meter,
                kind: Kind::Inline,
                map_key: self.map_key,
            },
        )
    }
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_any(Visit {
            inner: visitor,
            meter: self.meter,
            kind: Kind::Inline,
            map_key: self.map_key,
        })
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> std::result::Result<V::Value, Self::Error> {
        self.inner.deserialize_enum(
            name,
            variants,
            Visit {
                inner: visitor,
                meter: self.meter,
                kind: Kind::Inline,
                map_key: self.map_key,
            },
        )
    }
}
