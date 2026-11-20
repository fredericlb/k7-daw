// value.rs — a plain value and the JavaScript spellings the printer must match: numbers, strings, whitespace, UTF-16 widths.
// A reader would get wrong that a width is bytes: the TypeScript counts UTF-16 units, so every width here goes through `u16len`.

use crate::ast::PathSeg;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

/// Whether a key is a canonical array index, which a JS object lists first and in ascending order.
fn array_index(k: &str) -> Option<u32> {
    let ok =
        k == "0" || (!k.starts_with('0') && !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()));
    if !ok || k.len() > 10 {
        return None;
    }
    k.parse::<u32>().ok().filter(|&n| n != u32::MAX)
}

impl Value {
    /// A map ordered as a JS object would hold it; the last of a duplicate key wins, in the first one's place.
    #[must_use]
    pub fn object(entries: Vec<(String, Value)>) -> Value {
        let mut out: Vec<(String, Value)> = Vec::with_capacity(entries.len());
        for (k, v) in entries {
            match out.iter_mut().find(|(o, _)| *o == k) {
                Some(slot) => slot.1 = v,
                None => out.push((k, v)),
            }
        }
        let (mut idx, rest): (Vec<_>, Vec<_>) =
            out.into_iter().partition(|(k, _)| array_index(k).is_some());
        idx.sort_by_key(|(k, _)| array_index(k));
        idx.extend(rest);
        Value::Map(idx)
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// JS `typeof`: null, lists and maps are all "object".
    #[must_use]
    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            _ => "object",
        }
    }
}

/// Deep equality of plain data (applyData's `same`): `===` on scalars, so NaN differs from itself and -0 equals 0.
#[must_use]
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Num(x), Value::Num(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && same(v, v2)))
        }
        _ => false,
    }
}

/// A path as `Array.join(".")` writes it, for an error message.
#[must_use]
pub fn path_text(path: &[PathSeg]) -> String {
    path.iter()
        .map(|s| match s {
            PathSeg::Key(k) => k.clone(),
            PathSeg::Index(i) => i.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Length in UTF-16 code units, what JS `String.length` counts.
#[must_use]
pub fn u16len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The characters JS `\s`, `trim` and `trimEnd` treat as whitespace.
#[must_use]
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

#[must_use]
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

#[must_use]
pub fn js_trim_end(s: &str) -> &str {
    s.trim_end_matches(is_js_space)
}

/// JS `String(number)`: shortest round-trip digits, laid out as ECMAScript's Number::toString does.
#[must_use]
pub fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sgn = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{sgn}{}", e.abs())
        } else {
            format!("{}.{}e{sgn}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    format!("{sign}{body}")
}

/// JS `JSON.stringify(string)`: the short escapes, `\u00xx` for the other controls, everything else as it is.
#[must_use]
pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

/// JS `JSON.stringify(value)` for a value with no undefined: a non-finite number is `null`, -0 is `0`.
#[must_use]
pub fn json_text(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) if n.is_finite() => js_number(*n),
        Value::Num(_) => "null".into(),
        Value::Str(s) => json_string(s),
        Value::List(items) => {
            let parts: Vec<_> = items.iter().map(json_text).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Map(m) => {
            let parts: Vec<_> = m
                .iter()
                .map(|(k, v)| format!("{}:{}", json_string(k), json_text(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}
