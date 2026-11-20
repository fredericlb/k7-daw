// modules/unions.rs — the unions zod leaves open, each a hand-written visitor, and the map whose KEYS are checked.
// A reader would get wrong why none is an untagged enum: that buffers the node and answers "did not match any variant".

use super::scalars::KeyRule;
use indexmap::IndexMap;
use serde::de::{self, Deserialize, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;
use std::marker::PhantomData;

/// A param: a number, or the string of a file-valued one.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    Number(f64),
    Text(String),
}

impl Serialize for ParamValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ParamValue::Number(n) => s.serialize_f64(*n),
            ParamValue::Text(t) => s.serialize_str(t),
        }
    }
}

struct ParamVisitor;

impl Visitor<'_> for ParamVisitor {
    type Value = ParamValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number or a string")
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<ParamValue, E> {
        Ok(ParamValue::Number(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<ParamValue, E> {
        Ok(ParamValue::Number(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<ParamValue, E> {
        Ok(ParamValue::Number(v as f64))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<ParamValue, E> {
        Ok(ParamValue::Text(v.to_owned()))
    }
}

impl<'de> Deserialize<'de> for ParamValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ParamVisitor)
    }
}

/// An interface input: a preferences label, or one or two physical channel numbers.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Label(String),
    Channels(Vec<u64>),
}

impl Serialize for Input {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Input::Label(l) => s.serialize_str(l),
            Input::Channels(c) => {
                let mut seq = s.serialize_seq(Some(c.len()))?;
                for n in c {
                    seq.serialize_element(n)?;
                }
                seq.end()
            }
        }
    }
}

struct InputVisitor;

impl<'de> Visitor<'de> for InputVisitor {
    type Value = Input;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a label, or a list of one or two channel numbers")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Input, E> {
        if v.is_empty() {
            Err(E::custom("expected a non-empty string"))
        } else {
            Ok(Input::Label(v.to_owned()))
        }
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Input, A::Error> {
        let mut out: Vec<u64> = Vec::new();
        while let Some(n) = seq.next_element::<crate::files::scalar::PosInt>()? {
            if out.len() == 2 {
                return Err(de::Error::custom("expected a list of one or two channel numbers"));
            }
            out.push(n.0);
        }
        if out.is_empty() {
            return Err(de::Error::custom("expected a list of one or two channel numbers"));
        }
        Ok(Input::Channels(out))
    }
}

impl<'de> Deserialize<'de> for Input {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(InputVisitor)
    }
}

/// An input's monitoring: the tri-state, or a boolean for on and off.
#[derive(Clone, Debug, PartialEq)]
pub enum Monitor {
    Flag(bool),
    Mode(String),
}

impl Serialize for Monitor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Monitor::Flag(b) => s.serialize_bool(*b),
            Monitor::Mode(m) => s.serialize_str(m),
        }
    }
}

struct MonitorVisitor;

impl Visitor<'_> for MonitorVisitor {
    type Value = Monitor;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a boolean, or one of `off`, `auto`, `on`")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Monitor, E> {
        Ok(Monitor::Flag(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Monitor, E> {
        match v {
            "off" | "auto" | "on" => Ok(Monitor::Mode(v.to_owned())),
            _ => Err(E::custom("expected one of `off`, `auto`, `on`")),
        }
    }
}

impl<'de> Deserialize<'de> for Monitor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(MonitorVisitor)
    }
}

/// Which attribute set is on: its name, or its 1-based index.
#[derive(Clone, Debug, PartialEq)]
pub enum AlternativePick {
    Name(String),
    Index(u64),
}

impl Serialize for AlternativePick {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            AlternativePick::Name(n) => s.serialize_str(n),
            AlternativePick::Index(i) => s.serialize_u64(*i),
        }
    }
}

struct PickVisitor;

impl Visitor<'_> for PickVisitor {
    type Value = AlternativePick;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a name or a whole number of at least 1")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<AlternativePick, E> {
        if v.is_empty() {
            Err(E::custom("expected a non-empty string"))
        } else {
            Ok(AlternativePick::Name(v.to_owned()))
        }
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<AlternativePick, E> {
        u64::try_from(v)
            .ok()
            .filter(|n| *n >= 1)
            .map(AlternativePick::Index)
            .ok_or_else(|| E::custom("expected a whole number of at least 1"))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<AlternativePick, E> {
        if v >= 1 {
            Ok(AlternativePick::Index(v))
        } else {
            Err(E::custom("expected a whole number of at least 1"))
        }
    }
}

impl<'de> Deserialize<'de> for AlternativePick {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(PickVisitor)
    }
}

/// A map whose keys a rule checks, the error landing on the key.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyed<R, V>(pub IndexMap<String, V>, PhantomData<R>);

impl<R, V> Default for Keyed<R, V> {
    fn default() -> Self {
        Keyed(IndexMap::new(), PhantomData)
    }
}

impl<R, V> Keyed<R, V> {
    pub fn new(map: IndexMap<String, V>) -> Self {
        Keyed(map, PhantomData)
    }
}

impl<R, V: Serialize> Serialize for Keyed<R, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

struct KeySeed<R>(PhantomData<R>);

impl<'de, R: KeyRule> DeserializeSeed<'de> for KeySeed<R> {
    type Value = String;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<String, D::Error> {
        let key = String::deserialize(d)?;
        if R::ok(&key) {
            Ok(key)
        } else {
            Err(de::Error::custom(R::MESSAGE))
        }
    }
}

struct KeyedVisitor<R, V>(PhantomData<(R, V)>);

impl<'de, R: KeyRule, V: Deserialize<'de>> Visitor<'de> for KeyedVisitor<R, V> {
    type Value = Keyed<R, V>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Keyed<R, V>, A::Error> {
        let mut out = IndexMap::new();
        while let Some(k) = map.next_key_seed(KeySeed::<R>(PhantomData))? {
            out.insert(k, map.next_value::<V>()?);
        }
        Ok(Keyed(out, PhantomData))
    }
}

impl<'de, R: KeyRule, V: Deserialize<'de>> Deserialize<'de> for Keyed<R, V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(KeyedVisitor(PhantomData))
    }
}

/// A value where `null` means "the default", as a preprocessing step of the schema reads it.
pub struct NullOr<T>(pub Option<T>);

struct NullOrVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for NullOrVisitor<T> {
    type Value = NullOr<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_unit<E: de::Error>(self) -> Result<NullOr<T>, E> {
        Ok(NullOr(None))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<NullOr<T>, A::Error> {
        T::deserialize(de::value::MapAccessDeserializer::new(map)).map(|v| NullOr(Some(v)))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NullOr<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(NullOrVisitor(PhantomData))
    }
}
