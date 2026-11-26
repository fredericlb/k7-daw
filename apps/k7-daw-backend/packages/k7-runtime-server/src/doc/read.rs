//! read.rs — a JSON object read as nlohmann's `value(key, default)` read it: a field of the wrong type refused
//! with nlohmann's own words, an absent one defaulted, and a list that is absent or null an empty one.
//! What a reader gets wrong: a key present with a `null` is the wrong type, not absent, as in nlohmann; and a
//! list field that is neither an array nor null is refused, where nlohmann walked an object's values.

use crate::server::msg::Malformed;
use serde_json::{Map, Value};

pub type Read<T> = Result<T, Malformed>;

/// nlohmann's name for a value's type, as its errors print it.
pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// A `type_error.302`: a value of the wrong type.
pub fn wrong_type(want: &str, got: &Value) -> Malformed {
    Malformed(format!(
        "[json.exception.type_error.302] type must be {want}, but is {}",
        type_name(got)
    ))
}

/// A `out_of_range.401`: an array too short for an index read.
pub fn out_of_range(index: usize) -> Malformed {
    Malformed(format!(
        "[json.exception.out_of_range.401] array index {index} is out of range"
    ))
}

/// A `out_of_range.403`: a key an `at` needed.
pub fn missing_key(key: &str) -> Malformed {
    Malformed(format!(
        "[json.exception.out_of_range.403] key '{key}' not found"
    ))
}

/// A JSON number as an `int`, saturating past its range.
pub fn as_int(v: &Value) -> Read<i32> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) as i32),
        other => Err(wrong_type("number", other)),
    }
}

/// A JSON number as a `double`.
pub fn as_f64(v: &Value) -> Read<f64> {
    v.as_f64().ok_or_else(|| wrong_type("number", v))
}

/// One element of an array, as a number: the pair `[a, b]` of a marker or a point.
pub fn pair(v: &Value) -> Read<(f64, f64)> {
    let items = match v {
        Value::Array(a) => a.as_slice(),
        Value::Null => &[],
        other => return Err(wrong_type("array", other)),
    };
    let at = |i: usize| items.get(i).ok_or_else(|| out_of_range(i)).and_then(as_f64);
    Ok((at(0)?, at(1)?))
}

/// The object a `value()` call reads from.
#[derive(Clone, Copy)]
pub struct Obj<'a>(pub &'a Value);

impl<'a> Obj<'a> {
    fn map(&self) -> Read<&'a Map<String, Value>> {
        self.0.as_object().ok_or_else(|| {
            Malformed(format!(
                "[json.exception.type_error.306] cannot use value() with {}",
                type_name(self.0)
            ))
        })
    }

    /// The field if present, null included; `Err` when `self` is not an object.
    pub fn field(&self, key: &str) -> Read<Option<&'a Value>> {
        Ok(self.map()?.get(key))
    }

    /// `contains(key)`: false for anything but an object that has it.
    pub fn has(&self, key: &str) -> bool {
        self.0.as_object().is_some_and(|m| m.contains_key(key))
    }

    /// `at(key)`: the field, or the error nlohmann threw.
    pub fn at(&self, key: &str) -> Read<&'a Value> {
        match self.0 {
            Value::Object(m) => m.get(key).ok_or_else(|| missing_key(key)),
            other => Err(Malformed(format!(
                "[json.exception.type_error.304] cannot use at() with {}",
                type_name(other)
            ))),
        }
    }

    fn typed<T>(&self, key: &str, pick: impl FnOnce(&'a Value) -> Read<T>, default: T) -> Read<T> {
        match self.field(key)? {
            None => Ok(default),
            Some(v) => pick(v),
        }
    }

    pub fn str(&self, key: &str, default: &str) -> Read<String> {
        self.typed(
            key,
            |v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| wrong_type("string", v))
            },
            default.to_owned(),
        )
    }

    pub fn bool(&self, key: &str, default: bool) -> Read<bool> {
        self.typed(
            key,
            |v| v.as_bool().ok_or_else(|| wrong_type("boolean", v)),
            default,
        )
    }

    pub fn f64(&self, key: &str, default: f64) -> Read<f64> {
        self.typed(key, as_f64, default)
    }

    /// A `float` field, widened: the C++ reader narrows it back to the same value.
    pub fn f32(&self, key: &str, default: f32) -> Read<f64> {
        self.typed(
            key,
            |v| as_f64(v).map(|f| f64::from(f as f32)),
            f64::from(default),
        )
    }

    pub fn int(&self, key: &str, default: i32) -> Read<i32> {
        self.typed(key, as_int, default)
    }

    /// `get<vector<string>>`: an array of strings, each refused when it is not one.
    pub fn strings(&self, key: &str) -> Read<Vec<String>> {
        match self.field(key)? {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| wrong_type("string", v))
                })
                .collect(),
            Some(other) => Err(wrong_type("array", other)),
        }
    }

    /// A list field walked by a `for`: absent and null are empty, a non-array is refused.
    pub fn list(&self, key: &str) -> Read<&'a [Value]> {
        match self.field(key)? {
            None | Some(Value::Null) => Ok(&[]),
            Some(Value::Array(items)) => Ok(items),
            Some(other) => Err(wrong_type("array", other)),
        }
    }

    /// A list read only when it is an array, as `contains(k) && is_array()` read it.
    pub fn array_if(&self, key: &str) -> &'a [Value] {
        match self.0.as_object().and_then(|m| m.get(key)) {
            Some(Value::Array(items)) => items,
            _ => &[],
        }
    }
}
