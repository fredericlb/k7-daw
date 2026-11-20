//! strict.rs — the comparison of two answers: the first place two JSON values differ, in JavaScript's own order.
//! Objects compare key by key in the order `Object.keys` lists them, numbers as `f64`, `null` and absent apart.
//! What a reader gets wrong: `song::compare` ignores key order; this does not, since a client reads it (1006).

use serde_json::Value;

/// The nesting past which a comparison gives up; `serde_json` itself refuses documents deeper than 128.
const MAX_NESTING: usize = 512;
/// The bytes of each side a difference line shows.
pub const CUT: usize = 160;

/// What a comparison found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compared {
    Same,
    Differs {
        pointer: String,
        ts: String,
        native: String,
    },
    /// The budget of nodes ran out before the end.
    Capped,
}

/// The number an array-index key spells, which JavaScript lists first (`"0"`, never `"07"` or `"4294967295"`).
fn index_of(key: &str) -> Option<u32> {
    let canonical = key == "0" || (!key.starts_with('0') && !key.is_empty());
    if !canonical || key.len() > 10 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u32>().ok().filter(|n| *n != u32::MAX)
}

/// An object's own keys as `Object.keys` lists them: integer-like ascending, then the rest as inserted.
fn own_keys(m: &serde_json::Map<String, Value>) -> Vec<&String> {
    let mut keys: Vec<&String> = m.keys().collect();
    if keys.iter().any(|k| index_of(k).is_some()) {
        keys.sort_by_key(|k| index_of(k).map_or((1, 0), |n| (0, n)));
    }
    keys
}

/// A value as text, cut to `CUT` bytes without writing what is past the cut.
fn show(v: &Value) -> String {
    let mut out = String::new();
    show_into(v, &mut out);
    out
}

fn full(out: &str) -> bool {
    out.len() >= CUT
}

fn show_into(v: &Value, out: &mut String) {
    if full(out) {
        return;
    }
    match v {
        Value::String(s) => {
            let room = CUT.saturating_sub(out.len());
            let mut end = s.len().min(room);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            let cut = serde_json::to_string(&s[..end]).unwrap_or_default();
            if end < s.len() {
                out.push_str(&cut[..cut.len() - 1]);
                out.push_str("...\"");
            } else {
                out.push_str(&cut);
            }
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if full(out) {
                    out.push_str("...");
                    return;
                }
                if i > 0 {
                    out.push(',');
                }
                show_into(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if full(out) {
                    out.push_str("...");
                    return;
                }
                if i > 0 {
                    out.push(',');
                }
                show_into(&Value::String(k.clone()), out);
                out.push(':');
                show_into(x, out);
            }
            out.push('}');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

struct Walk {
    budget: usize,
    pointer: String,
}

enum Step {
    Same,
    Differs(String, String, String),
    Capped,
}

fn escaped(key: &str, into: &mut String) {
    into.push('/');
    for c in key.chars() {
        match c {
            '~' => into.push_str("~0"),
            '/' => into.push_str("~1"),
            c => into.push(c),
        }
    }
}

fn side(m: &serde_json::Map<String, Value>, key: &str) -> String {
    m.get(key).map_or("absent".into(), show)
}

impl Walk {
    fn differs(&self, ts: &str, native: &str) -> Step {
        Step::Differs(self.pointer.clone(), ts.to_string(), native.to_string())
    }

    fn at(&mut self, ts: &Value, native: &Value, depth: usize) -> Step {
        if self.budget == 0 || depth > MAX_NESTING {
            return Step::Capped;
        }
        self.budget -= 1;
        match (ts, native) {
            (Value::Null, Value::Null) => Step::Same,
            (Value::Bool(a), Value::Bool(b)) if a == b => Step::Same,
            (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => Step::Same,
            (Value::String(a), Value::String(b)) if a == b => Step::Same,
            (Value::Array(a), Value::Array(b)) => {
                for i in 0..a.len().max(b.len()) {
                    let was = self.pointer.len();
                    escaped(&i.to_string(), &mut self.pointer);
                    let step = match (a.get(i), b.get(i)) {
                        (Some(x), Some(y)) => self.at(x, y, depth + 1),
                        (x, y) => self.differs(
                            &x.map_or("absent".into(), show),
                            &y.map_or("absent".into(), show),
                        ),
                    };
                    if !matches!(step, Step::Same) {
                        return step;
                    }
                    self.pointer.truncate(was);
                }
                Step::Same
            }
            (Value::Object(a), Value::Object(b)) => {
                let (ka, kb) = (own_keys(a), own_keys(b));
                for i in 0..ka.len().max(kb.len()) {
                    let was = self.pointer.len();
                    let step = match (ka.get(i), kb.get(i)) {
                        (Some(x), Some(y)) if x == y => {
                            escaped(x, &mut self.pointer);
                            self.at(&a[*x], &b[*y], depth + 1)
                        }
                        (x, y) => {
                            let key = x.or(y).map_or("", |k| k.as_str());
                            escaped(key, &mut self.pointer);
                            let moved = x.zip(y).filter(|(x, y)| {
                                b.contains_key(x.as_str()) && a.contains_key(y.as_str())
                            });
                            match moved {
                                Some((x, y)) => self.differs(
                                    &format!("key {} at {i}", show(&Value::String((*x).clone()))),
                                    &format!("key {} at {i}", show(&Value::String((*y).clone()))),
                                ),
                                None => self.differs(&side(a, key), &side(b, key)),
                            }
                        }
                    };
                    if !matches!(step, Step::Same) {
                        return step;
                    }
                    self.pointer.truncate(was);
                }
                Step::Same
            }
            (a, b) => self.differs(&show(a), &show(b)),
        }
    }
}

/// The first difference of `ts` and `native`, or `Capped` once `budget` nodes were compared.
pub fn first_difference(ts: &Value, native: &Value, budget: usize) -> Compared {
    let mut w = Walk {
        budget,
        pointer: String::new(),
    };
    match w.at(ts, native, 0) {
        Step::Same => Compared::Same,
        Step::Differs(pointer, ts, native) => Compared::Differs {
            pointer,
            ts,
            native,
        },
        Step::Capped => Compared::Capped,
    }
}
