//! json.rs — `JSON.stringify` of a `serde_json::Value`, byte for byte: keys in their order, numbers spelt as
//! JavaScript spells them (`String(number)`), strings escaped as V8 escapes them. What `.cache/index.json` and a
//! listing are written with, since node reads and writes the same files; `pretty` is the `null, 2` form.

use serde_json::Value;

/// The text `JSON.stringify(value)` makes.
pub fn stringify(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

/// The text `JSON.stringify(value, null, 2)` makes.
pub fn pretty(value: &Value) -> String {
    let mut out = String::new();
    write_pretty(value, &mut out, 0);
    out
}

fn write_pretty(value: &Value, out: &mut String, depth: usize) {
    let pad = |out: &mut String, d: usize| out.push_str(&"  ".repeat(d));
    match value {
        Value::Array(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(out, depth + 1);
                write_pretty(v, out, depth + 1);
            }
            out.push('\n');
            pad(out, depth);
            out.push(']');
        }
        Value::Object(map) if !map.is_empty() => {
            out.push_str("{\n");
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(out, depth + 1);
                quote(k, out);
                out.push_str(": ");
                write_pretty(v, out, depth + 1);
            }
            out.push('\n');
            pad(out, depth);
            out.push('}');
        }
        other => write(other, out),
    }
}

fn write(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => out.push_str(&k7_js::number::to_string(f)),
            _ => out.push_str("null"),
        },
        Value::String(s) => quote(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(v, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(k, out);
                out.push(':');
                write(v, out);
            }
            out.push('}');
        }
    }
}

fn quote(s: &str, out: &mut String) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn indents_as_json_stringify_indents() {
        let v = json!({ "a": { "b": [1, {}], "c": [] }, "d": "x" });
        assert_eq!(
            pretty(&v),
            "{\n  \"a\": {\n    \"b\": [\n      1,\n      {}\n    ],\n    \"c\": []\n  },\n  \"d\": \"x\"\n}"
        );
        assert_eq!(pretty(&json!({})), "{}");
    }

    #[test]
    fn writes_what_json_stringify_writes() {
        let v = json!({ "b": 1.0, "a": [1.5, 1e21, -0.0, 0.000001, 1e-7], "s": "q\"\\\u{1}\n\u{2028}é", "n": null, "t": true });
        assert_eq!(
            stringify(&v),
            "{\"b\":1,\"a\":[1.5,1e+21,0,0.000001,1e-7],\"s\":\"q\\\"\\\\\\u0001\\n\u{2028}é\",\"n\":null,\"t\":true}"
        );
    }
}
