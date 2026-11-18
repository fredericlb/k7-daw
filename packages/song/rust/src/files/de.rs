// files/de.rs — a serde Deserializer over k7x's tree, so a file decodes straight into typed structs, its error located.
// A reader would get wrong where an error lands: the key of an unknown field, else the deepest node being decoded.

use k7x::{Entry, MapNode, Node, Scalar};
use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;
use std::fmt;

/// A decoding error and the byte offset it is about, once some node has claimed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeError {
    pub message: String,
    pub at: Option<usize>,
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DeError {}

/// What serde expected, in the words of a k7x file rather than Rust's types.
fn plain(expected: &str) -> String {
    match expected {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
            "a whole number".to_owned()
        }
        "f32" | "f64" => "a number".to_owned(),
        "a sequence" => "a list".to_owned(),
        "a map" => "an object".to_owned(),
        "unit" | "unit value" => "nothing".to_owned(),
        other => match other.strip_prefix("a tuple of size ") {
            Some(n) => format!("a list of {n} items"),
            None => other
                .strip_prefix("struct ")
                .map_or_else(|| other.to_owned(), |_| "an object".to_owned()),
        },
    }
}

/// What serde found, likewise.
fn found(unexpected: &de::Unexpected) -> String {
    use de::Unexpected as U;
    match unexpected {
        U::Bool(b) => b.to_string(),
        U::Unsigned(n) => format!("the number {n}"),
        U::Signed(n) => format!("the number {n}"),
        U::Float(n) => format!("the number {n}"),
        U::Str(s) => format!("the text \"{s}\""),
        U::Unit | U::Option => "nothing".to_owned(),
        U::Seq => "a list".to_owned(),
        U::Map => "an object".to_owned(),
        U::NewtypeStruct | U::Enum | U::UnitVariant | U::NewtypeVariant | U::TupleVariant | U::StructVariant => {
            "a value of another shape".to_owned()
        }
        U::Char(_) | U::Bytes(_) => "a value of another kind".to_owned(),
        U::Other(o) => (*o).to_owned(),
    }
}

fn list_of(names: &[&str]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

// Every constructor serde calls is overridden, so a message never names a Rust type.
impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeError {
            message: msg.to_string(),
            at: None,
        }
    }

    fn invalid_type(unexpected: de::Unexpected, expected: &dyn de::Expected) -> Self {
        Self::custom(format!(
            "expected {}, found {}",
            plain(&expected.to_string()),
            found(&unexpected)
        ))
    }

    fn invalid_value(unexpected: de::Unexpected, expected: &dyn de::Expected) -> Self {
        Self::custom(format!(
            "{} is out of range: expected {}",
            found(&unexpected),
            plain(&expected.to_string())
        ))
    }

    fn invalid_length(len: usize, expected: &dyn de::Expected) -> Self {
        Self::custom(format!(
            "expected {}, found a list of {len}",
            plain(&expected.to_string())
        ))
    }

    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        Self::custom(format!("`{variant}` is not one of {}", list_of(expected)))
    }

    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        if expected.is_empty() {
            Self::custom(format!("unknown key `{field}`"))
        } else {
            Self::custom(format!("unknown key `{field}`, expected one of {}", list_of(expected)))
        }
    }

    fn missing_field(field: &'static str) -> Self {
        Self::custom(format!("missing key `{field}`"))
    }

    fn duplicate_field(field: &'static str) -> Self {
        Self::custom(format!("key `{field}` is written twice"))
    }
}

impl DeError {
    fn or_at(mut self, at: usize) -> Self {
        self.at.get_or_insert(at);
        self
    }
}

/// Decodes the root map into `T`; the error carries the offset it is placed on.
pub fn from_map<T: de::DeserializeOwned>(root: &MapNode) -> Result<T, DeError> {
    T::deserialize(NodeDe(Node::Map(*root))).map_err(|e| e.or_at(root.start))
}

struct NodeDe<'a>(Node<'a>);

fn number<'de, V: Visitor<'de>>(n: f64, v: V) -> Result<V::Value, DeError> {
    if !n.is_finite() {
        return Err(de::Error::custom("expected a finite number"));
    }
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 {
        v.visit_i64(n as i64)
    } else {
        v.visit_f64(n)
    }
}

fn str_de(s: &str) -> de::value::StrDeserializer<'_, DeError> {
    s.into_deserializer()
}

impl<'de> de::Deserializer<'de> for NodeDe<'_> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let at = self.0.start();
        let out = match self.0 {
            Node::Scalar { value, .. } => match value {
                Scalar::Str(s) => visitor.visit_str(s),
                Scalar::Num(n) => number(n, visitor),
                Scalar::Bool(b) => visitor.visit_bool(b),
                Scalar::Null => visitor.visit_unit(),
            },
            Node::List { items, .. } => {
                let mut acc = SeqAcc { items, at: 0 };
                match visitor.visit_seq(&mut acc) {
                    Ok(_) if acc.at < items.len() => {
                        Err(de::Error::invalid_length(items.len(), &"fewer elements in the list"))
                    }
                    other => other,
                }
            }
            Node::Map(m) => visitor.visit_map(MapAcc {
                entries: m.entries,
                at: 0,
            }),
        };
        out.map_err(|e| e.or_at(at))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        // A null is a value, not an absence: the schema refuses `null` where a number goes.
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        let at = self.0.start();
        match self.0 {
            Node::Scalar {
                value: Scalar::Str(s), ..
            } => visitor.visit_enum(str_de(s)).map_err(|e| e.or_at(at)),
            _ => Err(DeError {
                message: "expected a name".to_owned(),
                at: Some(at),
            }),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        unit unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

struct SeqAcc<'a> {
    items: &'a [Node<'a>],
    at: usize,
}

impl<'de> SeqAccess<'de> for SeqAcc<'_> {
    type Error = DeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, DeError> {
        let Some(node) = self.items.get(self.at).copied() else {
            return Ok(None);
        };
        self.at += 1;
        seed.deserialize(NodeDe(node))
            .map(Some)
            .map_err(|e| e.or_at(node.start()))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len() - self.at)
    }
}

struct MapAcc<'a> {
    entries: &'a [Entry<'a>],
    at: usize,
}

impl<'de> MapAccess<'de> for MapAcc<'_> {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, DeError> {
        let Some(e) = self.entries.get(self.at).copied() else {
            return Ok(None);
        };
        seed.deserialize(str_de(e.key))
            .map(Some)
            .map_err(|er| er.or_at(e.key_start))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let Some(e) = self.entries.get(self.at).copied() else {
            return Err(de::Error::custom("a value with no key"));
        };
        self.at += 1;
        seed.deserialize(NodeDe(e.value))
            .map_err(|er| er.or_at(e.value.start()))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len() - self.at)
    }
}
