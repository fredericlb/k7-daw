//! yaml.rs — a legacy `preferences.yaml` read as the `yaml` library's core schema would: a tree of k7x values, or none when it does not read.
//! What a reader gets wrong: only a plain scalar is resolved; a duplicate key, a second document or a complex key is a file that does not read.

use std::collections::HashMap;

use k7x::edit::Value;
use saphyr_parser::{Event, Parser, ScalarStyle};

/// A text longer than this does not read (the file is a few hundred bytes).
const MAX_TEXT: usize = 1 << 20;
/// Collections nested deeper than this do not read.
const MAX_DEPTH: usize = 64;

/// The text as `yaml` reads it: no byte order mark, `\r\n` one line break.
fn prepared(text: &str) -> String {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
}

fn number(t: &str) -> Option<f64> {
    let (sign, body) = match t.as_bytes().first()? {
        b'-' => (-1.0, &t[1..]),
        b'+' => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    if let Some(d) = t
        .strip_prefix("0x")
        .filter(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return u64::from_str_radix(d, 16).ok().map(|n| n as f64);
    }
    if let Some(d) = t
        .strip_prefix("0o")
        .filter(|d| !d.is_empty() && d.bytes().all(|c| (b'0'..=b'7').contains(&c)))
    {
        return u64::from_str_radix(d, 8).ok().map(|n| n as f64);
    }
    match body {
        ".inf" | ".Inf" | ".INF" => return Some(sign * f64::INFINITY),
        _ => {}
    }
    if matches!(t, ".nan" | ".NaN" | ".NAN") {
        return Some(f64::NAN);
    }
    let digits = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    let (mant, exp) = body
        .split_once(['e', 'E'])
        .map_or((body, None), |(m, e)| (m, Some(e)));
    let mant_ok = match mant.split_once('.') {
        Some((i, f)) => digits(i) && digits(f) && !(i.is_empty() && f.is_empty()),
        None => !mant.is_empty() && digits(mant),
    };
    let exp_ok = exp.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    (mant_ok && exp_ok).then(|| t.parse::<f64>().ok()).flatten()
}

/// A plain scalar by the core schema.
fn plain(t: &str) -> Value {
    match t {
        "" | "~" | "null" | "Null" | "NULL" => Value::Null,
        "true" | "True" | "TRUE" => Value::Bool(true),
        "false" | "False" | "FALSE" => Value::Bool(false),
        _ => number(t).map_or_else(|| Value::Str(t.to_owned()), Value::Num),
    }
}

enum Frame {
    Seq(Vec<Value>, usize),
    Map(Vec<(String, Value)>, Option<String>, usize),
}

/// The document's data, or none when it does not read; an empty document is null.
pub fn parse(text: &str) -> Option<Value> {
    if text.len() > MAX_TEXT {
        return None;
    }
    let text = prepared(text);
    let mut parser = Parser::new_from_str(&text);
    let (mut stack, mut root, mut docs): (Vec<Frame>, Option<Value>, usize) = (Vec::new(), None, 0);
    let mut anchors: HashMap<usize, Value> = HashMap::new();
    while let Some(next) = parser.next_event() {
        let (event, _) = next.ok()?;
        let (done, anchor): (Value, usize) = match event {
            Event::DocumentStart(_) => {
                docs += 1;
                if docs > 1 {
                    return None;
                }
                continue;
            }
            Event::Scalar(t, style, anchor, _) => {
                let v = if style == ScalarStyle::Plain {
                    plain(&t)
                } else {
                    Value::Str(t.into_owned())
                };
                (v, anchor)
            }
            Event::Alias(id) => (anchors.get(&id)?.clone(), 0),
            Event::SequenceStart(anchor, _) => {
                if stack.len() >= MAX_DEPTH {
                    return None;
                }
                stack.push(Frame::Seq(Vec::new(), anchor));
                continue;
            }
            Event::MappingStart(anchor, _) => {
                if stack.len() >= MAX_DEPTH {
                    return None;
                }
                stack.push(Frame::Map(Vec::new(), None, anchor));
                continue;
            }
            Event::SequenceEnd => match stack.pop()? {
                Frame::Seq(v, a) => (Value::List(v), a),
                Frame::Map(..) => return None,
            },
            Event::MappingEnd => match stack.pop()? {
                Frame::Map(v, _, a) => (Value::Map(v), a),
                Frame::Seq(..) => return None,
            },
            _ => continue,
        };
        if anchor > 0 {
            anchors.insert(anchor, done.clone());
        }
        match stack.last_mut() {
            None => root = Some(done),
            Some(Frame::Seq(v, _)) => v.push(done),
            Some(Frame::Map(v, key, _)) => match key.take() {
                None => {
                    let k = match done {
                        Value::Str(s) => s,
                        Value::Num(n) => k7x::edit::js_number(n),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_owned(),
                        _ => return None,
                    };
                    if v.iter().any(|(e, _)| *e == k) {
                        return None;
                    }
                    *key = Some(k);
                }
                Some(k) => v.push((k, done)),
            },
        }
    }
    Some(root.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_mapping_reads_by_the_core_schema() {
        let v = parse(
            "audioDevice:\n  name: \"Focusrite\"\neditor: {vim: false}\ncount: 0x10\nlabel: yes\n",
        )
        .unwrap();
        let Value::Map(m) = v else { panic!() };
        assert_eq!(m[2], ("count".to_owned(), Value::Num(16.0)));
        assert_eq!(m[3], ("label".to_owned(), Value::Str("yes".to_owned())));
    }

    #[test]
    fn what_does_not_read_is_none() {
        for bad in [
            "a: [1",
            "a: 1\na: 2\n",
            "---\na: 1\n---\nb: 2\n",
            "? [x]\n: y\n",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
        assert_eq!(parse("").unwrap(), Value::Null);
    }
}
