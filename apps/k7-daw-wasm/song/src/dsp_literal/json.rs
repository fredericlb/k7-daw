// dsp_literal/json.rs — the graph JSON as `JSON.stringify(graph, null, 2)` prints it, plus the newline the jail adds.
// A reader would get wrong that serde_json would do: it prints `60` as `60.0`, and key order here is the builder's, not sorted.

use crate::clips::js_number;

/// A value of the graph JSON, objects keeping the order their keys were written in.
pub(crate) enum J {
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(&'static str, J)>),
}

/// The text of `jsdsp.json`: two-space indents, one item per line, a trailing newline.
pub(crate) fn pretty(j: &J) -> String {
    let mut out = String::new();
    write(j, 0, &mut out);
    out.push('\n');
    out
}

fn indent(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn write(j: &J, depth: usize, out: &mut String) {
    match j {
        J::Num(n) => out.push_str(&js_number(*n)),
        J::Str(s) => quote(s, out),
        J::Arr(items) if items.is_empty() => out.push_str("[]"),
        J::Obj(entries) if entries.is_empty() => out.push_str("{}"),
        J::Arr(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                indent(depth + 1, out);
                write(item, depth + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            indent(depth, out);
            out.push(']');
        }
        J::Obj(entries) => {
            out.push_str("{\n");
            for (i, (key, value)) in entries.iter().enumerate() {
                indent(depth + 1, out);
                quote(key, out);
                out.push_str(": ");
                write(value, depth + 1, out);
                out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
            }
            indent(depth, out);
            out.push('}');
        }
    }
}

/// A string as `JSON.stringify` quotes it: the short escapes, other controls as lowercase `\u00xx`.
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
