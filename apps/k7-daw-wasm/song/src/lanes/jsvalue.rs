// lanes/jsvalue.rs — a JSON value read as JavaScript's `Number(v)` and `String(v)` read it: what the rig's channels and labels go through.
// A reader would get wrong that this is the TypeScript's coercion, not a validation: `"3"`, `" 4 "`, `true` and `[3]` are all numbers.

use crate::clips::js_number;
use crate::files::scalar::js_space;
use serde_json::Value;

/// How deep `String(array)` follows nested arrays; serde_json's own parser refuses anything deeper.
const DEPTH: usize = 128;

fn radix_number(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut acc = 0.0_f64;
    for c in digits.chars() {
        let Some(d) = c.to_digit(radix) else { return f64::NAN };
        acc = acc * f64::from(radix) + f64::from(d);
    }
    acc
}

/// `Number(s)` of a string: trimmed, empty is 0, `0x`/`0o`/`0b` radix literals, `Infinity`, else a decimal literal or NaN.
fn string_number(s: &str) -> f64 {
    let t = s.trim_matches(js_space);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = t.get(..2).filter(|p| p.eq_ignore_ascii_case(prefix)).and(t.get(2..)) {
            return radix_number(rest, radix);
        }
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    // Rust's parser also reads `inf`, `nan` and `infinity`; JavaScript reads none of them.
    if !t
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// `Number(v)`: a number as it stands, `true` 1, `null` and `false` 0, a string or an array through its text, an object NaN.
pub fn to_number(v: &Value) -> f64 {
    match v {
        Value::Null | Value::Bool(false) => 0.0,
        Value::Bool(true) => 1.0,
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => string_number(s),
        Value::Array(_) => string_number(&to_text(v)),
        Value::Object(_) => f64::NAN,
    }
}

fn text_at(v: &Value, depth: usize, out: &mut String) {
    match v {
        Value::Null => {}
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&js_number(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => out.push_str(s),
        Value::Object(_) => out.push_str("[object Object]"),
        Value::Array(list) if depth < DEPTH => {
            for (i, e) in list.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                text_at(e, depth + 1, out);
            }
        }
        Value::Array(_) => {}
    }
}

/// `String(v)`: `null` is `"null"` as a value but an array's `null` is empty, and an array is its elements joined by commas.
pub fn to_text(v: &Value) -> String {
    let mut out = String::new();
    match v {
        Value::Null => out.push_str("null"),
        other => text_at(other, 0, &mut out),
    }
    out
}
