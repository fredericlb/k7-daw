// js_text.rs — the JavaScript spellings a port must match: `trim`, `Number(x)` and `String(x)` over a k7x value.
// A reader would get wrong that a missing key is not `null`: `Number(undefined)` is NaN and `Number(null)` is 0, so callers pass `Option`.

use k7x::edit::{Value, js_number};

/// The characters JS `trim` strips.
#[must_use]
pub fn is_js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

#[must_use]
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `Number(string)`: empty is 0, a `0x`, `0o` or `0b` prefix is a radix, `Infinity` spelled as JS spells it, anything else NaN.
#[must_use]
pub fn str_to_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let radix = |p: &str, r: u32| {
        t.get(..2)
            .filter(|h| h.eq_ignore_ascii_case(p))
            .map(|_| u64::from_str_radix(&t[2..], r).map_or(f64::NAN, |n| n as f64))
    };
    if let Some(n) = radix("0x", 16).or_else(|| radix("0o", 8)).or_else(|| radix("0b", 2)) {
        return n;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let plain = t
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'));
    if plain {
        t.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `String(value)` for a value a file holds.
#[must_use]
pub fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) => js_number(*n),
        Value::Str(s) => s.clone(),
        Value::List(l) => l
            .iter()
            .map(|x| {
                if matches!(x, Value::Null) {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Map(_) => "[object Object]".to_owned(),
    }
}

/// `Number(value)`; None stands for `undefined`.
#[must_use]
pub fn js_to_number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Num(n)) => *n,
        Some(Value::Str(s)) => str_to_number(s),
        Some(other @ Value::List(_)) => str_to_number(&js_string(other)),
        Some(Value::Map(_)) => f64::NAN,
    }
}

/// `Number.isInteger(n) && n > 0`.
#[must_use]
pub fn is_positive_integer(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0 && n > 0.0
}
