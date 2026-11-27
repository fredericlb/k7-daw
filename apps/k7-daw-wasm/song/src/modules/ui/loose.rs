// loose.rs — a panel as loose JSON: the widget editor holds widgets mid-edit, so the rules read them as JavaScript does.
// A reader would get wrong that these are not the strict `Widget` types: a field may be missing or a number a fraction, and a rule must still answer.

use serde_json::{Map, Number, Value};

/// A JSON object, its keys in the order the page wrote them.
pub type Obj = Map<String, Value>;

/// The number under `key`, NaN when it is missing or no number (so a comparison on it is false, as `undefined`'s is).
pub fn num(w: &Value, key: &str) -> f64 {
    w.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// The text under `key`.
pub fn text<'a>(w: &'a Value, key: &str) -> Option<&'a str> {
    w.get(key).and_then(Value::as_str)
}

/// A widget's `type`, empty when it has none.
pub fn kind(w: &Value) -> &str {
    text(w, "type").unwrap_or("")
}

/// `Math.max(1, n ?? 1)`: a widget's span, 1 when it says none.
pub fn span(w: &Value, key: &str) -> f64 {
    w.get(key).and_then(Value::as_f64).map_or(1.0, |n| n.max(1.0))
}

/// A count as the JSON number JavaScript would print: an integer when it is one.
pub fn jnum(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9e15 {
        Value::Number(Number::from(n as i64))
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

/// The object under `key`.
pub fn obj<'a>(w: &'a Value, key: &str) -> Option<&'a Obj> {
    w.get(key).and_then(Value::as_object)
}

/// `{...w}` with `key` set: an existing key keeps its place, a new one goes last.
pub fn with(w: &Value, key: &str, value: Value) -> Value {
    let mut out = w.as_object().cloned().unwrap_or_default();
    out.insert(key.to_owned(), value);
    Value::Object(out)
}

/// A widget map's `(id, widget)` pairs in the map's order.
pub fn entries(m: Option<&Obj>) -> Vec<(&String, &Value)> {
    m.map(|m| m.iter().collect()).unwrap_or_default()
}

/// A truthy `bind`: JavaScript's `w.bind &&` on a string.
pub fn bind(w: &Value) -> Option<&str> {
    text(w, "bind").filter(|b| !b.is_empty())
}
