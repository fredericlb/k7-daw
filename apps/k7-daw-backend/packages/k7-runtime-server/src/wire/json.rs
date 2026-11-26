//! json.rs — JSON as nlohmann's `dump()` prints it, what every client reads: compact, an object's keys sorted by
//! their bytes, a float in nlohmann's own format (`1.0`, `0.0001`, `1e-05`, `1e+15`), non-ASCII raw.
//! What a reader gets wrong: serde_json's own printer differs on the last two points and on key order
//! (`preserve_order`), so every frame the server sends goes through `dump`; `fixtures/nlohmann_dump.txt` is
//! the truth both printers are held to (`tests/all/wire_json.rs`, and k7-runtime's `WireJson_test.cpp`).

use serde_json::{Number, Value};
use std::fmt::Write;

/// The text of `v`, as `json::dump(-1, ' ', false, replace)` wrote it.
pub fn dump(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v);
    out
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, &map[key.as_str()]);
            }
            out.push('}');
        }
    }
}

fn write_number(out: &mut String, n: &Number) {
    if let Some(u) = n.as_u64() {
        let _ = write!(out, "{u}");
    } else if let Some(i) = n.as_i64() {
        let _ = write!(out, "{i}");
    } else {
        write_f64(out, n.as_f64().unwrap_or(f64::NAN));
    }
}

/// A double as nlohmann's `to_chars` prints it: shortest digits, placed by `format_buffer` (-4 and 15).
pub fn write_f64(out: &mut String, x: f64) {
    if !x.is_finite() {
        out.push_str("null");
        return;
    }
    if x.is_sign_negative() {
        out.push('-');
    }
    let x = x.abs();
    if x == 0.0 {
        out.push_str("0.0");
        return;
    }
    let (digits, decimal_exponent) = shortest(x);
    let k = digits.len() as i32;
    let n = k + decimal_exponent;
    const MIN_EXP: i32 = -4;
    const MAX_EXP: i32 = 15;
    if k <= n && n <= MAX_EXP {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
        out.push_str(".0");
    } else if 0 < n && n <= MAX_EXP {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if MIN_EXP < n && n <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-n) as usize));
        out.push_str(&digits);
    } else {
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = n - 1;
        out.push('e');
        out.push(if e < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", e.abs());
    }
}

/// The shortest round-trip digits of a positive finite `x`, and the power of ten of the last one.
fn shortest(x: f64) -> (String, i32) {
    let text = format!("{x:e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let decimal_exponent = exponent - (digits.len() as i32 - 1);
    (digits, decimal_exponent)
}

/// A string as nlohmann escapes it with `ensure_ascii` off: the two-letter escapes, `\u00xx` below 0x20.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
