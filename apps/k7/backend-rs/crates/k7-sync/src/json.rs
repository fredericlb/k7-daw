//! json.rs — `JSON.stringify` byte for byte: numbers as `String(number)`, strings as V8 escapes them, and every
//! object's keys in a JavaScript object's order, where an array index comes first, ascending.
//! What a reader gets wrong: that order holds for a parsed object too, so it is applied to every object written.

use std::cmp::Ordering;

use serde_json::{Map, Value};

/// The text `JSON.stringify(value)` makes.
pub fn stringify(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out, None, 0);
    out
}

/// The text `JSON.stringify(value, null, indent)` makes.
pub fn stringify_indented(value: &Value, indent: usize) -> String {
    let mut out = String::new();
    write(value, &mut out, Some(indent), 0);
    out
}

/// A key JavaScript orders first: a canonical array index, below 2^32 - 1.
fn index_of(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|n| *n < u32::MAX as u64)
        .map(|n| n as u32)
}

/// An object's keys in the order a JavaScript object holds them.
pub fn js_keys(map: &Map<String, Value>) -> Vec<&String> {
    js_order(map.keys())
}

/// Keys in insertion order, as an object made of them orders them.
pub fn js_order<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut indices: Vec<(u32, &String)> = Vec::new();
    let mut names: Vec<&String> = Vec::new();
    for k in keys {
        match index_of(k) {
            Some(i) => indices.push((i, k)),
            None => names.push(k),
        }
    }
    if indices.is_empty() {
        return names;
    }
    indices.sort_by_key(|(i, _)| *i);
    indices.into_iter().map(|(_, k)| k).chain(names).collect()
}

fn newline(out: &mut String, indent: Option<usize>, depth: usize) {
    if let Some(n) = indent {
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', n * depth));
    }
}

fn write(value: &Value, out: &mut String, indent: Option<usize>, depth: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => out.push_str(&k7_js::number::to_string(f)),
            _ => out.push_str("null"),
        },
        Value::String(s) => quote(s, out),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write(v, out, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, k) in js_keys(map).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                quote(k, out);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write(&map[k], out, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push('}');
        }
    }
}

/// A string as `JSON.stringify` quotes it.
pub fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A string's `length`: its UTF-16 code units.
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The order `Array.prototype.sort` gives strings: by UTF-16 code units.
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `decodeURIComponent`, None where it throws a URIError.
pub fn decode_uri_component(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A number field as JavaScript reads it: any JSON number, as a double.
pub fn num(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn writes_what_json_stringify_writes() {
        let v = json!({ "b": 1.0, "a": [1.5, 1e21, -0.0, 0.000001, 1e-7], "s": "q\"\\\u{1}\n\u{2028}é", "n": null, "t": true });
        assert_eq!(
            stringify(&v),
            "{\"b\":1,\"a\":[1.5,1e+21,0,0.000001,1e-7],\"s\":\"q\\\"\\\\\\u0001\\n\u{2028}é\",\"n\":null,\"t\":true}"
        );
    }

    #[test]
    fn orders_array_indices_first() {
        let v = json!({ "b": 1, "10": 2, "a": 3, "2": 4, "01": 5, "4294967295": 6 });
        assert_eq!(
            stringify(&v),
            r#"{"2":4,"10":2,"b":1,"a":3,"01":5,"4294967295":6}"#
        );
    }

    #[test]
    fn indents_as_json_stringify_does() {
        let v = json!([{ "seq": 1, "name": "a", "e": [], "o": {} }]);
        assert_eq!(
            stringify_indented(&v, 1),
            "[\n {\n  \"seq\": 1,\n  \"name\": \"a\",\n  \"e\": [],\n  \"o\": {}\n }\n]"
        );
        assert_eq!(stringify_indented(&json!([]), 1), "[]");
    }

    #[test]
    fn uri_components() {
        assert_eq!(encode_uri_component("a b/é%"), "a%20b%2F%C3%A9%25");
        assert_eq!(
            decode_uri_component("a%20b%2F%C3%A9%25").as_deref(),
            Some("a b/é%")
        );
        assert_eq!(decode_uri_component("%C3"), None);
        assert_eq!(decode_uri_component("%zz"), None);
        assert_eq!(decode_uri_component("%ED%A0%80"), None);
    }

    #[test]
    fn utf16_order() {
        assert_eq!(utf16_cmp("\u{FFFF}", "\u{10000}"), Ordering::Greater);
        assert_eq!(utf16_len("a\u{10000}"), 3);
    }
}
