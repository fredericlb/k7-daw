// views/nullable.rs — a value where `null` is itself a value: the one wrapper a surface's mapping entries need.
// A reader would get wrong that an Option refuses `null` here (files/de.rs): this type answers it, and nothing else may.

use serde::de::value::{
    BoolDeserializer, F64Deserializer, I64Deserializer, MapAccessDeserializer, SeqAccessDeserializer,
};
use serde::de::value::{StrDeserializer, U64Deserializer};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;

/// `null` is `Nullable(None)`; any other shape is read as a `T`.
#[derive(Clone, Debug, PartialEq)]
pub struct Nullable<T>(pub Option<T>);

impl<T: Serialize> Serialize for Nullable<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Some(v) => v.serialize(s),
            None => s.serialize_unit(),
        }
    }
}

struct Seen<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for Seen<T> {
    type Value = Nullable<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a value or null")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Nullable(None))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        T::deserialize(BoolDeserializer::<E>::new(v)).map(|t| Nullable(Some(t)))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        T::deserialize(I64Deserializer::<E>::new(v)).map(|t| Nullable(Some(t)))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        T::deserialize(U64Deserializer::<E>::new(v)).map(|t| Nullable(Some(t)))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        T::deserialize(F64Deserializer::<E>::new(v)).map(|t| Nullable(Some(t)))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        T::deserialize(StrDeserializer::<E>::new(v)).map(|t| Nullable(Some(t)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        T::deserialize(SeqAccessDeserializer::new(a)).map(|t| Nullable(Some(t)))
    }

    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(a)).map(|t| Nullable(Some(t)))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Seen(PhantomData))
    }
}
