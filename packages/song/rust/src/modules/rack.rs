// modules/rack.rs — a module's rack: its two flavours by `rack_type`, each pad a TAP with its note and its sample.
// A reader would get wrong the defaults: a missing or null `rack_type` is `samples` and `symbols` an empty kit, before the flavour is chosen.

use super::unions::NullOr;
use crate::files::scalar::{MidiKey, NonEmpty, NonNeg, Pos};
use indexmap::IndexMap;
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

/// One rack sound: the note it answers to, plus a library `sample`, or a clip, or a window of the source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RackSound {
    pub note: MidiKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<NonEmpty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<NonEmpty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<NonNeg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Pos>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RackType {
    Samples,
    Slice,
}

impl Serialize for RackType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            RackType::Samples => "samples",
            RackType::Slice => "slice",
        })
    }
}

/// A rack: `samples` (taps on library samples) or `slice` (taps on windows of one source).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rack {
    pub rack_type: RackType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub symbols: IndexMap<String, RackSound>,
}

const FIELDS: [&str; 3] = ["rack_type", "source", "symbols"];

/// A key of a rack, refused on the key itself when it is none of the three.
struct FieldSeed;

impl<'de> DeserializeSeed<'de> for FieldSeed {
    type Value = String;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<String, D::Error> {
        let key = String::deserialize(d)?;
        if FIELDS.contains(&key.as_str()) {
            Ok(key)
        } else {
            Err(de::Error::custom(format!(
                "not a rack field: {key} — a rack's pads go under `symbols:`"
            )))
        }
    }
}

/// `rack_type`: absent or null is `samples`.
struct TypeSeed;

struct TypeVisitor;

impl Visitor<'_> for TypeVisitor {
    type Value = RackType;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("`samples` or `slice`")
    }

    fn visit_unit<E: de::Error>(self) -> Result<RackType, E> {
        Ok(RackType::Samples)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<RackType, E> {
        match v {
            "samples" => Ok(RackType::Samples),
            "slice" => Ok(RackType::Slice),
            _ => Err(E::custom("a rack_type is `samples` or `slice`")),
        }
    }
}

impl<'de> DeserializeSeed<'de> for TypeSeed {
    type Value = RackType;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<RackType, D::Error> {
        d.deserialize_any(TypeVisitor)
    }
}

struct RackVisitor;

impl<'de> Visitor<'de> for RackVisitor {
    type Value = Rack;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Rack, E> {
        Ok(Rack {
            rack_type: RackType::Samples,
            source: None,
            symbols: IndexMap::new(),
        })
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Rack, A::Error> {
        let (mut rack_type, mut source, mut symbols) = (RackType::Samples, None, IndexMap::new());
        while let Some(key) = map.next_key_seed(FieldSeed)? {
            match key.as_str() {
                "rack_type" => rack_type = map.next_value_seed(TypeSeed)?,
                "source" => source = Some(map.next_value::<NonEmpty>()?.0),
                _ => {
                    symbols = map
                        .next_value::<NullOr<IndexMap<String, RackSound>>>()?
                        .0
                        .unwrap_or_default()
                }
            }
        }
        match rack_type {
            RackType::Slice if source.is_none() => return Err(de::Error::custom("a slice rack needs a `source`")),
            RackType::Slice if symbols.values().any(|s| s.from.is_none() || s.to.is_none()) => {
                return Err(de::Error::custom("a slice rack's pads each need `from` and `to`"));
            }
            _ => {}
        }
        Ok(Rack {
            rack_type,
            source,
            symbols,
        })
    }
}

impl<'de> Deserialize<'de> for Rack {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(RackVisitor)
    }
}
