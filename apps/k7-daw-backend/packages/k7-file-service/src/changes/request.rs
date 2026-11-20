//! request.rs — the body of `POST /changes` made into a `Request`: every field's type, the kinds of op, the limits.
//! What a reader gets wrong: `was` has no default, a missing key is a 400 and `null` means "the file is absent".

use serde_json::{Map, Value};

use crate::journal::Mode;

use super::{MAX_LABEL, MAX_OPS, MAX_TEXT, Op, Refusal, Request};

fn bad(s: String) -> Refusal {
    Refusal::Bad(s)
}

fn text<'a>(o: &'a Map<String, Value>, at: &str, key: &str) -> Result<&'a str, Refusal> {
    match o.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Ok(s),
        Some(Value::String(_)) => Err(bad(format!("{at}{key}: must not be empty"))),
        _ => Err(bad(format!("{at}{key}: expected a string"))),
    }
}

fn flag(o: &Map<String, Value>, at: &str, key: &str) -> Result<bool, Refusal> {
    match o.get(key) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(bad(format!("{at}{key}: expected a boolean"))),
    }
}

fn was(o: &Map<String, Value>, at: &str) -> Result<Option<String>, Refusal> {
    match o.get("was") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(bad(format!("{at}was: expected a string or null"))),
    }
}

fn op(i: usize, v: &Value) -> Result<Op, Refusal> {
    let at = format!("ops[{i}].");
    let o = v
        .as_object()
        .ok_or_else(|| bad(format!("ops[{i}]: expected an object")))?;
    let kind = text(o, &at, "kind")?;
    let file = || text(o, &at, "file").map(String::from);
    Ok(match kind {
        "text" => {
            let Some(Value::String(t)) = o.get("text") else {
                return Err(bad(format!("{at}text: expected a string")));
            };
            Op::Text {
                file: file()?,
                text: t.clone(),
                was: was(o, &at)?,
            }
        }
        "remove" => Op::Remove {
            file: file()?,
            was: was(o, &at)?,
        },
        "read" => Op::Read {
            file: file()?,
            was: was(o, &at)?,
        },
        "copy" | "move" => {
            let (from, file, replace) = (
                text(o, &at, "from")?.to_string(),
                file()?,
                flag(o, &at, "replace")?,
            );
            if kind == "copy" {
                Op::Copy {
                    from,
                    file,
                    replace,
                }
            } else {
                Op::Move {
                    from,
                    file,
                    replace,
                }
            }
        }
        "adopt" => Op::Adopt {
            job: text(o, &at, "job")?.to_string(),
            out: text(o, &at, "out")?.to_string(),
            file: file()?,
            replace: flag(o, &at, "replace")?,
        },
        other => return Err(bad(format!("{at}kind: unknown kind '{other}'"))),
    })
}

/// The body made into a request; the count of ops and the weight of their text are judged before any op is read.
pub fn parse(body: &Value) -> Result<Request, Refusal> {
    let o = body
        .as_object()
        .ok_or_else(|| bad("expected an object".into()))?;
    // An empty `at` is the workspace alone (no project), the one field that may be empty.
    let at = match o.get("at") {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(bad("at: expected a string".into())),
    };
    let (base, label) = (
        text(o, "", "base")?.to_string(),
        text(o, "", "label")?.to_string(),
    );
    if label.chars().count() > MAX_LABEL {
        return Err(bad(format!("label: longer than {MAX_LABEL} characters")));
    }
    let journal = match o.get("journal") {
        None => Mode::Step,
        Some(Value::String(s)) if s == "join" => Mode::Join,
        Some(Value::String(s)) if s == "skip" => Mode::Skip,
        Some(_) => return Err(bad("journal: expected 'join' or 'skip'".into())),
    };
    let want_graph = match o.get("want") {
        None => false,
        Some(Value::Array(w)) if w.iter().all(|x| x == "graph") => !w.is_empty(),
        Some(_) => return Err(bad("want: expected a list of 'graph'".into())),
    };
    let create = match o.get("create") {
        None => None,
        Some(Value::String(s)) => Some(super::landing::folder_name(&at, s)?),
        Some(_) => return Err(bad("create: expected a folder name".into())),
    };
    let Some(Value::Array(ops)) = o.get("ops") else {
        return Err(bad("ops: expected an array".into()));
    };
    if ops.len() > MAX_OPS {
        return Err(Refusal::TooMany(format!(
            "a change set carries at most {MAX_OPS} ops"
        )));
    }
    let weight = ops
        .iter()
        .filter_map(|v| v.get("text").and_then(Value::as_str))
        .fold(0usize, |n, t| n.saturating_add(t.len()));
    if weight > MAX_TEXT {
        return Err(Refusal::TooMany(format!(
            "a change set carries at most {MAX_TEXT} bytes of text"
        )));
    }
    let ops = ops
        .iter()
        .enumerate()
        .map(|(i, v)| op(i, v))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Request {
        at,
        base,
        label,
        journal,
        ops,
        create,
        reload: flag(o, "", "reload")?,
        want_graph,
    })
}
