//! rig.rs — the rig a compile is told: `normalizePreferences`' channels and cue pair, read from `<studio>/preferences.k7x`.
//! What a reader gets wrong: nothing is written, and a file only a migration can read (the YAML to upgrade) is declined; Number() and String() are JavaScript's.

use std::io::Read;

use serde_json::{Value, json};

use crate::filter::code_of;
use crate::libraries::Home;
use crate::paths::join;
use crate::read::{ReadError, open_regular};
use crate::space::Declined;

const PREFS_FILE: &str = "preferences.k7x";
/// The most a preferences file is read at; a larger one is declined rather than held.
const MAX_BYTES: u64 = 64 << 20;

/// A k7x value as the JavaScript parse hands it to `normalizePreferences`.
enum J {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    List(Vec<J>),
    Map(Vec<(String, J)>),
}

fn from_node(n: &k7x::Node<'_>) -> J {
    match n {
        k7x::Node::Scalar { value, .. } => match value {
            k7x::Scalar::Str(s) => J::Str((*s).to_owned()),
            k7x::Scalar::Num(x) => J::Num(*x),
            k7x::Scalar::Bool(b) => J::Bool(*b),
            k7x::Scalar::Null => J::Null,
        },
        k7x::Node::List { items, .. } => J::List(items.iter().map(from_node).collect()),
        k7x::Node::Map(m) => J::Map(
            m.entries
                .iter()
                .map(|e| (e.key.to_owned(), from_node(&e.value)))
                .collect(),
        ),
    }
}

fn get<'a>(v: &'a J, key: &str) -> Option<&'a J> {
    match v {
        J::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

/// JavaScript's WhiteSpace and LineTerminator, which is not Rust's `is_whitespace` (U+FEFF is one, U+0085 is not).
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

/// `Number(string)`: the StringNumericLiteral grammar, `NaN` for anything else.
fn string_to_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let radix = |digits: &str, base: u32| {
        if digits.is_empty() {
            return f64::NAN;
        }
        let mut acc = 0.0_f64;
        for c in digits.chars() {
            match c.to_digit(base) {
                Some(d) => acc = acc * f64::from(base) + f64::from(d),
                None => return f64::NAN,
            }
        }
        acc
    };
    if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return radix(rest, 16);
    }
    if let Some(rest) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
        return radix(rest, 8);
    }
    if let Some(rest) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
        return radix(rest, 2);
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'-' => (-1.0, &t[1..]),
        b'+' => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !decimal_literal(body) {
        return f64::NAN;
    }
    body.parse::<f64>().map_or(f64::NAN, |n| sign * n)
}

/// `digits [. digits] [e[+-]digits]` with either side of the dot optional but not both.
fn decimal_literal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };
    let whole = digits(&mut i);
    let mut fraction = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        fraction = digits(&mut i);
    }
    if whole + fraction == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

/// `String(n)` for a number, by ECMAScript's Number::toString.
fn number_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n == 0.0 {
        return "0".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if n < 0.0 {
        return format!("-{}", number_string(-n));
    }
    let sci = format!("{n:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let k = digits.len() as i32;
    let point = exp.parse::<i32>().unwrap_or(0) + 1;
    if k <= point && point <= 21 {
        return format!("{digits}{}", "0".repeat((point - k) as usize));
    }
    if 0 < point && point <= 21 {
        let (a, b) = digits.split_at(point as usize);
        return format!("{a}.{b}");
    }
    if -6 < point && point <= 0 {
        return format!("0.{}{digits}", "0".repeat((-point) as usize));
    }
    let e = point - 1;
    let sign = if e < 0 { '-' } else { '+' };
    let head = &digits[..1];
    if k == 1 {
        format!("{head}e{sign}{}", e.abs())
    } else {
        format!("{head}.{}e{sign}{}", &digits[1..], e.abs())
    }
}

/// `String(v)` of a value reached through `Array.prototype.join`, where null is empty.
fn element_string(v: &J) -> String {
    match v {
        J::Null => String::new(),
        v => js_string(v),
    }
}

/// `String(v)`.
fn js_string(v: &J) -> String {
    match v {
        J::Str(s) => s.clone(),
        J::Num(n) => number_string(*n),
        J::Bool(b) => b.to_string(),
        J::Null => "null".into(),
        J::List(items) => items
            .iter()
            .map(element_string)
            .collect::<Vec<_>>()
            .join(","),
        J::Map(_) => "[object Object]".into(),
    }
}

/// `Number(v)`.
fn js_number(v: &J) -> f64 {
    match v {
        J::Num(n) => *n,
        J::Bool(b) => f64::from(u8::from(*b)),
        J::Null => 0.0,
        v => string_to_number(&js_string(v)),
    }
}

/// A positive integer as JSON carries the number a JavaScript `JSON.stringify` wrote for it.
fn positive_integer(v: &J) -> Option<Value> {
    let n = js_number(v);
    if !(n.is_finite() && n.fract() == 0.0 && n > 0.0) {
        return None;
    }
    let text = if n < 1e21 {
        format!("{n}")
    } else {
        format!("{n:e}")
    };
    serde_json::from_str(&text).ok()
}

fn channel_list(v: &J, dir: &str) -> Vec<Value> {
    let Some(J::List(list)) = get(v, "channels").and_then(|c| get(c, dir)) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|e| {
            let ch = match get(e, "ch") {
                Some(J::List(items)) => items.iter().filter_map(positive_integer).collect(),
                Some(other) => positive_integer(other).into_iter().collect(),
                None => Vec::new(),
            };
            let label = match get(e, "label") {
                None | Some(J::Null) => String::new(),
                Some(l) => js_string(l),
            };
            (!ch.is_empty() && !label.is_empty()).then(|| json!({ "ch": ch, "label": label }))
        })
        .collect()
}

/// `{channels, cueOutput?}` of a parsed preferences mapping, by `normalizePreferences`' rules for those keys.
fn normalize(v: &J) -> Value {
    let mut rig =
        json!({ "channels": { "in": channel_list(v, "in"), "out": channel_list(v, "out") } });
    let cue = match get(v, "cueOutput") {
        Some(J::List(items)) => {
            let ch: Vec<Value> = items.iter().filter_map(positive_integer).collect();
            (!ch.is_empty()).then(|| Value::Array(ch))
        }
        Some(J::Str(s)) => Some(js_trim(s)).filter(|t| !t.is_empty()).map(Value::from),
        _ => None,
    };
    if let (Some(cue), Some(map)) = (cue, rig.as_object_mut()) {
        map.insert("cueOutput".into(), cue);
    }
    rig
}

/// The rig of no preferences: the defaults' shape, not `{}`.
pub fn empty_rig() -> Value {
    json!({ "channels": { "in": [], "out": [] } })
}

/// The rig of a k7x text: a mapping is read, a text that does not parse is the empty rig, one with a lone surrogate is declined.
pub fn rig_of_text(text: &str) -> Result<Value, Declined> {
    let bump = k7x::Bump::new();
    let Ok(parsed) = k7x::parse_tree_lossless(text, &bump) else {
        return Ok(empty_rig());
    };
    let Some(root) = parsed else {
        return Err(Declined::LossyText);
    };
    Ok(normalize(&from_node(&k7x::Node::Map(root))))
}

enum State {
    Text(String),
    Absent,
    Unreadable,
}

/// `readText`: the text whole, absent for `ENOENT` and `ENOTDIR`, anything else unreadable.
fn state_of(path: &str) -> Result<State, Declined> {
    match std::fs::metadata(path) {
        Err(e) if matches!(code_of(&e), "ENOENT" | "ENOTDIR") => return Ok(State::Absent),
        Err(_) => return Ok(State::Unreadable),
        Ok(m) if m.is_dir() => return Ok(State::Unreadable),
        Ok(m) if m.len() > MAX_BYTES => return Err(Declined::TooLarge),
        Ok(_) => {}
    }
    let file = match open_regular(path) {
        Ok(f) => f,
        Err(ReadError::Io {
            code: "ENOENT" | "ENOTDIR",
        }) => return Ok(State::Absent),
        Err(ReadError::Declined(d)) => return Err(d),
        Err(_) => return Ok(State::Unreadable),
    };
    let mut bytes = Vec::new();
    if file.take(MAX_BYTES + 1).read_to_end(&mut bytes).is_err() {
        return Ok(State::Unreadable);
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Declined::TooLarge);
    }
    Ok(State::Text(String::from_utf8_lossy(&bytes).into_owned()))
}

/// The rig `songRig()` answers (1511): `preferences.k7x` read as `find` reads it, and never written.
pub fn rig(home: &Home) -> Result<Value, Declined> {
    let file = join(&[&home.studio, PREFS_FILE]);
    match state_of(&file)? {
        State::Text(t) if !t.is_empty() => return rig_of_text(&t),
        State::Unreadable => return Ok(empty_rig()),
        State::Absent if std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_symlink()) => {
            return Ok(empty_rig());
        }
        State::Absent | State::Text(_) => {}
    }
    Ok(empty_rig())
}

/// `{audioDevice?: {name, type?}, inputPairs}` of a parsed preferences mapping, by `normalizePreferences`' rules for those keys.
fn engine_normalize(v: &J) -> Value {
    let device = match get(v, "audioDevice") {
        Some(J::Str(s)) if !s.is_empty() => Some(json!({ "name": s })),
        Some(d @ J::Map(_)) => match get(d, "name") {
            Some(J::Str(name)) if !name.is_empty() => {
                let mut out = json!({ "name": name });
                if let Some(J::Str(t)) =
                    get(d, "type").filter(|t| matches!(t, J::Str(s) if !s.is_empty()))
                {
                    out["type"] = json!(t);
                }
                Some(out)
            }
            _ => None,
        },
        _ => None,
    };
    let mut pairs: Vec<i64> = match get(v, "inputPairs") {
        Some(J::List(items)) => items
            .iter()
            .map(js_number)
            .filter(|n| n.fract() == 0.0 && *n > 0.0 && *n < 1e9)
            .map(|n| n as i64)
            .map(|n| if n % 2 == 1 { n } else { n - 1 })
            .collect(),
        _ => vec![],
    };
    pairs.sort_unstable();
    pairs.dedup();
    let mut out = json!({ "inputPairs": pairs });
    if let Some(d) = device {
        out["audioDevice"] = d;
    }
    out
}

/// What a socket's open replays (2215): the audio device and the grouped input pairs, read as the rig is, never written.
pub fn engine_prefs(home: &Home) -> Result<Value, Declined> {
    let file = join(&[&home.studio, PREFS_FILE]);
    let none = || json!({ "inputPairs": [] });
    match state_of(&file)? {
        State::Text(t) if !t.is_empty() => {
            let bump = k7x::Bump::new();
            let Ok(parsed) = k7x::parse_tree_lossless(&t, &bump) else {
                return Ok(none());
            };
            let Some(root) = parsed else {
                return Err(Declined::LossyText);
            };
            return Ok(engine_normalize(&from_node(&k7x::Node::Map(root))));
        }
        State::Unreadable => return Ok(none()),
        State::Absent if std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_symlink()) => {
            return Ok(none());
        }
        State::Absent | State::Text(_) => {}
    }
    Ok(none())
}

/// The two chain settings `loadPreferences` hands (1p-D12): a string trimmed, empty or not a string is none.
pub fn chain_settings(home: &Home) -> Result<(Option<String>, Option<String>), Declined> {
    let file = join(&[&home.studio, PREFS_FILE]);
    let text = match state_of(&file)? {
        State::Text(t) if !t.is_empty() => t,
        State::Unreadable => return Ok((None, None)),
        State::Absent if std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_symlink()) => {
            return Ok((None, None));
        }
        State::Absent | State::Text(_) => return Ok((None, None)),
    };
    let bump = k7x::Bump::new();
    let Ok(parsed) = k7x::parse_tree_lossless(&text, &bump) else {
        return Ok((None, None));
    };
    let Some(root) = parsed else {
        return Err(Declined::LossyText);
    };
    let v = from_node(&k7x::Node::Map(root));
    let pick = |key: &str| match get(&v, key) {
        Some(J::Str(s)) => Some(js_trim(s))
            .filter(|t| !t.is_empty())
            .map(str::to_owned),
        _ => None,
    };
    Ok((pick("stemsChain"), pick("instrumentalChain")))
}
