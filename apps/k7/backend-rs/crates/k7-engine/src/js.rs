//! js.rs — the JavaScript semantics the registry's answers and files are written in: a number as
//! `JSON.stringify` prints it, a string cut in UTF-16 units, and zod's messages for the few checks used.

use serde_json::{Number, Value};

/// A number as JavaScript holds it, back as JSON: integral values without a fraction, as `JSON.stringify` prints them.
pub fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 {
        Value::from(n as i64)
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

/// Every number of `v` re-read as JavaScript holds it, and an object's index keys first, so serde prints what `JSON.stringify` prints.
pub fn as_js(v: &Value) -> Value {
    match v {
        Value::Number(n) => n.as_f64().map_or(Value::Null, num),
        Value::Array(a) => Value::Array(a.iter().map(as_js).collect()),
        Value::Object(o) => {
            let mut index: Vec<(u32, &String, &Value)> = o
                .iter()
                .filter_map(|(k, v)| array_index(k).map(|i| (i, k, v)))
                .collect();
            index.sort_by_key(|(i, _, _)| *i);
            let named = o.iter().filter(|(k, _)| array_index(k).is_none());
            let keys = index.into_iter().map(|(_, k, v)| (k, v)).chain(named);
            Value::Object(keys.map(|(k, v)| (k.clone(), as_js(v))).collect())
        }
        other => other.clone(),
    }
}

/// A key JavaScript orders as an array index: a canonical integer below 2^32 - 1.
fn array_index(k: &str) -> Option<u32> {
    let n: u32 = k.parse().ok()?;
    (n != u32::MAX && n.to_string() == k).then_some(n)
}

/// `JSON.stringify(v)`.
pub fn stringify(v: &Value) -> String {
    serde_json::to_string(&as_js(v)).unwrap_or_default()
}

/// JavaScript's `\s`, which `trim()` strips: not Rust's `char::is_whitespace` (U+0085 and U+FEFF differ).
pub fn is_space(c: char) -> bool {
    matches!(c, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// `String.prototype.trim`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `String.prototype.length`: UTF-16 code units.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(0, n)` in UTF-16 units, a pair cut in its middle dropped whole (a Rust string holds no lone surrogate).
pub fn cut_utf16(s: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &s[..i];
        }
    }
    s
}

/// The name zod gives a received value; None is `undefined`.
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

/// zod's message for a value of the wrong type.
pub fn invalid_type(want: &str, got: Option<&Value>) -> String {
    format!("Invalid input: expected {want}, received {}", zod_type(got))
}

/// A string rule zod reports with its own message.
pub type Pattern<'a> = (&'a dyn Fn(&str) -> bool, &'a str);

/// `z.string().min(min).max(max)` and an optional pattern: every issue, as zod lists them.
pub fn string_issues(
    v: Option<&Value>,
    min: usize,
    max: usize,
    pattern: Option<Pattern<'_>>,
) -> Result<String, Vec<String>> {
    let Some(Value::String(s)) = v else {
        return Err(vec![invalid_type("string", v)]);
    };
    let len = utf16_len(s);
    let mut issues = Vec::new();
    if len < min {
        issues.push(format!(
            "Too small: expected string to have >={min} characters"
        ));
    }
    if len > max {
        issues.push(format!(
            "Too big: expected string to have <={max} characters"
        ));
    }
    if let Some((ok, message)) = pattern
        && !ok(s)
    {
        issues.push(message.to_string());
    }
    if issues.is_empty() {
        Ok(s.clone())
    } else {
        Err(issues)
    }
}

const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/// `z.number()`, `.int()` when `int`, then `.min(min)` and `.max(max)`: every issue, as zod lists them.
pub fn number_issues(v: Option<&Value>, int: bool, min: f64, max: f64) -> Result<f64, Vec<String>> {
    let Some(n) = v.and_then(Value::as_f64) else {
        return Err(vec![invalid_type("number", v)]);
    };
    let mut issues = Vec::new();
    if int {
        if n.fract() != 0.0 {
            return Err(vec!["Invalid input: expected int, received number".into()]);
        }
        if n > MAX_SAFE {
            issues.push("Too big: expected int to be <=9007199254740991".into());
        }
        if n < -MAX_SAFE {
            issues.push("Too small: expected int to be >=-9007199254740991".into());
        }
    }
    if n < min {
        issues.push(format!("Too small: expected number to be >={}", fmt(min)));
    }
    if n > max {
        issues.push(format!("Too big: expected number to be <={}", fmt(max)));
    }
    if issues.is_empty() {
        Ok(n)
    } else {
        Err(issues)
    }
}

/// A bound as JavaScript prints it in a message.
fn fmt(n: f64) -> String {
    stringify(&num(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_print_as_javascript() {
        assert_eq!(
            stringify(&json!({ "a": 5.0, "b": 1.5, "c": [2.0] })),
            r#"{"a":5,"b":1.5,"c":[2]}"#
        );
        assert_eq!(cut_utf16("ab🎛c", 3), "ab");
        assert_eq!(cut_utf16("abc", 3), "abc");
    }
}
