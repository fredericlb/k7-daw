//! js.rs — the few JavaScript semantics the ported routes must reproduce to answer as the TypeScript does:
//! `trim()`'s whitespace, `.length` in UTF-16 units, truthiness, and the type names zod's messages use.

use serde_json::Value;

/// JavaScript's `\s`, which `trim()` also strips: not Rust's `char::is_whitespace` (U+0085 and U+FEFF differ).
pub fn is_js_space(c: char) -> bool {
    matches!(c, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// `String.prototype.trim`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `String.prototype.length`: UTF-16 code units.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JavaScript's truthiness of a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// The name zod gives a received value in `Invalid input: expected …, received <name>`; None is `undefined`.
pub fn zod_type(v: Option<&Value>) -> &'static str {
    match v {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

/// zod's `expected <want>` message for a value of the wrong type.
pub fn invalid_type(want: &str, got: Option<&Value>) -> String {
    format!("Invalid input: expected {want}, received {}", zod_type(got))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_as_javascript() {
        assert_eq!(trim("\u{feff} a \u{3000}"), "a");
        assert_eq!(trim("\u{85}a"), "\u{85}a");
        assert_eq!(utf16_len("é🎛"), 3);
    }
}
