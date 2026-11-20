//! format.rs — the journal as the TypeScript writes it: `parse` checks 1724's shape, `print` gives `JSON.stringify`'s bytes
//! (1721). What a reader gets wrong: key order is the file's (integer-like keys first, as JavaScript orders them), and a
//! key this build does not know stays at its place; a number prints as `String(n)` does, not as serde prints it.

use serde_json::{Map, Number, Value};

type Obj = Map<String, Value>;

/// A journal that is not JSON, or not the shape (1724); the reason, for the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Torn(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Process,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Process => "process",
        }
    }
}

/// A text change (`before` and `after` are contents) or a blob change (they are file names under `blobs/`).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    /// The file's key order, empty for a change made here.
    pub order: Vec<String>,
    pub rest: Obj,
}

pub type BlobChange = Change;

impl Change {
    pub fn new(path: &str, before: Option<String>, after: Option<String>) -> Change {
        Change {
            path: path.into(),
            before,
            after,
            order: vec![],
            rest: Obj::new(),
        }
    }
}

/// The engine a step's blobs were handed to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineRef {
    pub id: String,
    pub name: String,
    pub order: Vec<String>,
    pub rest: Obj,
}

impl EngineRef {
    pub fn new(id: &str, name: &str) -> EngineRef {
        EngineRef {
            id: id.into(),
            name: name.into(),
            order: vec![],
            rest: Obj::new(),
        }
    }

    /// Whether `other` is the same engine: the id decides, the name is only what an error shows.
    pub fn is(&self, other: &EngineRef) -> bool {
        self.id == other.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u64,
    pub ts: Number,
    pub kind: Kind,
    pub label: String,
    pub changes: Vec<Change>,
    pub blobs: Vec<BlobChange>,
    /// Absent, `null` (kept here), or an engine.
    pub kept_on: Option<Option<EngineRef>>,
    pub order: Vec<String>,
    pub rest: Obj,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Journal {
    pub next_id: u64,
    pub cursor: usize,
    pub entries: Vec<Entry>,
    pub order: Vec<String>,
    pub rest: Obj,
}

impl Journal {
    pub fn empty() -> Journal {
        Journal {
            next_id: 1,
            cursor: 0,
            entries: vec![],
            order: vec![],
            rest: Obj::new(),
        }
    }
}

// --- JavaScript's number and key rules ----------------------------------------------------------------------------------

/// `String(x)` for a finite number: the shortest digits, laid out by ECMAScript's Number::toString.
pub fn js_number(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if x < 0.0 {
        return format!("-{}", js_number(-x));
    }
    let sci = format!("{x:e}");
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    }
}

fn number_string(n: &Number) -> String {
    js_number(n.as_f64().unwrap_or(0.0))
}

/// `JSON.stringify`'s string: `"` `\` and the controls escaped, nothing else.
pub fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A canonical array index (0 to 2^32 - 2), which JavaScript lists first in an object, ascending.
fn index_of(k: &str) -> Option<u64> {
    let canonical =
        !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) && (k == "0" || !k.starts_with('0'));
    canonical
        .then(|| k.parse::<u64>().ok())
        .flatten()
        .filter(|n| *n <= 4_294_967_294)
}

/// Keys as an object of JavaScript lists them.
fn js_order<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<String> {
    let (mut idx, other): (Vec<&String>, Vec<&String>) = keys.partition(|k| index_of(k).is_some());
    idx.sort_by_key(|k| index_of(k));
    idx.into_iter().chain(other).cloned().collect()
}

/// Any JSON value printed as `JSON.stringify` prints it.
pub fn print_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number_string(n)),
        Value::String(s) => quote(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                print_value(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, k) in js_order(m.keys()).iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(k, out);
                out.push(':');
                print_value(&m[k], out);
            }
            out.push('}');
        }
    }
}

// --- parse --------------------------------------------------------------------------------------------------------------

fn id_of(v: &Value) -> Option<u64> {
    let f = v.as_number()?.as_f64()?;
    (f >= 0.0 && f.fract() == 0.0 && f <= 9_007_199_254_740_991.0).then_some(f as u64)
}

fn text_or_null(v: Value) -> Option<Option<String>> {
    match v {
        Value::Null => Some(None),
        Value::String(s) => Some(Some(s)),
        _ => None,
    }
}

fn string(v: Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s),
        _ => None,
    }
}

fn object(v: Value) -> Option<(Vec<String>, Obj)> {
    let Value::Object(m) = v else { return None };
    Some((js_order(m.keys()), m))
}

fn change_of(v: Value) -> Option<Change> {
    let (order, mut m) = object(v)?;
    Some(Change {
        path: string(m.shift_remove("path")?)?,
        before: text_or_null(m.shift_remove("before")?)?,
        after: text_or_null(m.shift_remove("after")?)?,
        order,
        rest: m,
    })
}

fn engine_of(v: Value) -> Option<EngineRef> {
    let (order, mut m) = object(v)?;
    Some(EngineRef {
        id: string(m.shift_remove("id")?)?,
        name: string(m.shift_remove("name")?)?,
        order,
        rest: m,
    })
}

fn changes_of(v: Value) -> Option<Vec<Change>> {
    let Value::Array(a) = v else { return None };
    a.into_iter().map(change_of).collect()
}

fn entry_of(v: Value, next_id: u64) -> Option<Entry> {
    let (order, mut m) = object(v)?;
    let id = id_of(&m.shift_remove("id")?).filter(|id| *id < next_id)?;
    let Value::Number(ts) = m.shift_remove("ts")? else {
        return None;
    };
    if !ts.as_f64()?.is_finite() {
        return None;
    }
    let kind = match string(m.shift_remove("kind")?)?.as_str() {
        "text" => Kind::Text,
        "process" => Kind::Process,
        _ => return None,
    };
    let label = string(m.shift_remove("label")?)?;
    let changes = changes_of(m.shift_remove("changes")?)?;
    let blobs = changes_of(m.shift_remove("blobs")?)?;
    let kept_on = match m.shift_remove("keptOn") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(engine_of(v)?)),
    };
    Some(Entry {
        id,
        ts,
        kind,
        label,
        changes,
        blobs,
        kept_on,
        order,
        rest: m,
    })
}

/// The journal `text` holds, or why it is torn (1724).
pub fn parse(text: &str) -> Result<Journal, Torn> {
    let torn = |why: &str| Torn(why.to_string());
    let v: Value = serde_json::from_str(text).map_err(|e| Torn(e.to_string()))?;
    let (order, mut m) = object(v).ok_or_else(|| torn("not an object"))?;
    let next_id = m
        .shift_remove("nextId")
        .as_ref()
        .and_then(id_of)
        .ok_or_else(|| torn("nextId"))?;
    let cursor = m
        .shift_remove("cursor")
        .as_ref()
        .and_then(id_of)
        .ok_or_else(|| torn("cursor"))? as usize;
    let Some(Value::Array(list)) = m.shift_remove("entries") else {
        return Err(torn("entries"));
    };
    if cursor > list.len() {
        return Err(torn("cursor past the entries"));
    }
    let entries: Option<Vec<Entry>> = list.into_iter().map(|e| entry_of(e, next_id)).collect();
    let entries = entries.ok_or_else(|| torn("an entry"))?;
    Ok(Journal {
        next_id,
        cursor,
        entries,
        order,
        rest: m,
    })
}

// --- print --------------------------------------------------------------------------------------------------------------

/// An object whose known keys are written by `known` (it answers whether the key is there); the rest where the file had them.
fn write_object(
    out: &mut String,
    order: &[String],
    literal: &[&str],
    rest: &Obj,
    mut known: impl FnMut(&str, &mut String) -> bool,
) {
    let rest_keys = js_order(rest.keys());
    let keys: Vec<&str> = if order.is_empty() {
        literal
            .iter()
            .copied()
            .chain(rest_keys.iter().map(String::as_str))
            .collect()
    } else {
        order
            .iter()
            .map(String::as_str)
            .chain(
                literal
                    .iter()
                    .copied()
                    .filter(|k| !order.iter().any(|o| o == k)),
            )
            .collect()
    };
    out.push('{');
    let mut first = true;
    for k in keys {
        let mark = out.len();
        if !first {
            out.push(',');
        }
        quote(k, out);
        out.push(':');
        let wrote = if literal.contains(&k) {
            known(k, out)
        } else if let Some(v) = rest.get(k) {
            print_value(v, out);
            true
        } else {
            false
        };
        if wrote {
            first = false;
        } else {
            out.truncate(mark);
        }
    }
    out.push('}');
}

fn write_text(out: &mut String, t: &Option<String>) {
    match t {
        Some(s) => quote(s, out),
        None => out.push_str("null"),
    }
}

fn write_change(out: &mut String, c: &Change) {
    write_object(
        out,
        &c.order,
        &["path", "before", "after"],
        &c.rest,
        |k, out| {
            match k {
                "path" => quote(&c.path, out),
                "before" => write_text(out, &c.before),
                _ => write_text(out, &c.after),
            }
            true
        },
    );
}

fn write_changes(out: &mut String, list: &[Change]) {
    out.push('[');
    for (i, c) in list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_change(out, c);
    }
    out.push(']');
}

fn write_engine(out: &mut String, e: &EngineRef) {
    write_object(out, &e.order, &["id", "name"], &e.rest, |k, out| {
        quote(if k == "id" { &e.id } else { &e.name }, out);
        true
    });
}

fn write_entry(out: &mut String, e: &Entry) {
    let literal = ["id", "ts", "kind", "label", "changes", "blobs", "keptOn"];
    write_object(out, &e.order, &literal, &e.rest, |k, out| {
        match k {
            "id" => out.push_str(&e.id.to_string()),
            "ts" => out.push_str(&number_string(&e.ts)),
            "kind" => quote(e.kind.as_str(), out),
            "label" => quote(&e.label, out),
            "changes" => write_changes(out, &e.changes),
            "blobs" => write_changes(out, &e.blobs),
            _ => match &e.kept_on {
                None => return false,
                Some(None) => out.push_str("null"),
                Some(Some(r)) => write_engine(out, r),
            },
        }
        true
    });
}

/// `JSON.stringify(journal)`.
pub fn print(j: &Journal) -> String {
    let mut out = String::new();
    write_object(
        &mut out,
        &j.order,
        &["nextId", "cursor", "entries"],
        &j.rest,
        |k, out| {
            match k {
                "nextId" => out.push_str(&j.next_id.to_string()),
                "cursor" => out.push_str(&j.cursor.to_string()),
                _ => {
                    out.push('[');
                    for (i, e) in j.entries.iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        write_entry(out, e);
                    }
                    out.push(']');
                }
            }
            true
        },
    );
    out
}
