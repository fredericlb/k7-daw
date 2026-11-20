//! apply.rs — a k7x op's replay and its settle against the room's text (`applyK7x`, and `settleFileOp`'s k7x arm).
//! A replay splices a changed value's source verbatim and inserts through the k7x document edits song's writers use.
//! What a reader gets wrong: a malformed op fails as the TypeScript's `TypeError` did, since only a local caller sends one.

use k7_js::JsError;
use k7x::edit::{Document, EditError, Value, WriteOptions};
use k7x::{Bump, Node, PathSeg, parse_tree};
use serde_json::Value as Json;

use super::locate::{SegCache, Work, gone_at, locate, value_at};
use crate::settle::Settled;

fn error(name: &str, message: impl Into<String>) -> JsError {
    JsError {
        name: name.into(),
        message: message.into(),
    }
}

fn type_error(what: &str) -> JsError {
    error(
        "TypeError",
        format!("Cannot read properties of undefined (reading '{what}')"),
    )
}

fn edit_error(e: EditError) -> JsError {
    match e {
        EditError::Syntax(s) => error("K7SyntaxError", s.to_string()),
        EditError::Edit(m) => error("Error", m),
    }
}

/// What an op says of the value it replaced (`was`): absent, null, or a source.
#[derive(Clone, Copy, PartialEq)]
enum Was<'v> {
    Missing,
    Null,
    Text(&'v str),
    Other,
}

fn was_of(o: &Json) -> Was<'_> {
    match o.get("was") {
        None => Was::Missing,
        Some(Json::Null) => Was::Null,
        Some(Json::String(s)) => Was::Text(s),
        Some(_) => Was::Other,
    }
}

/// One op read from the wire; `after` is None for absent, Some(None) for null.
struct Op<'v> {
    set: bool,
    del: bool,
    path: Vec<&'v str>,
    k7x: Option<&'v str>,
    after: Option<Option<&'v str>>,
}

fn path_of(o: &Json) -> Result<Vec<&str>, JsError> {
    let Some(list) = o.get("path").and_then(Json::as_array) else {
        return Err(type_error("length"));
    };
    list.iter()
        .map(|s| s.as_str().ok_or_else(|| type_error("length")))
        .collect()
}

fn op_of(o: &Json) -> Result<Op<'_>, JsError> {
    let kind = o.get("op").and_then(Json::as_str);
    Ok(Op {
        set: kind == Some("set"),
        del: kind == Some("delete"),
        path: path_of(o)?,
        k7x: o.get("k7x").and_then(Json::as_str),
        after: match o.get("after") {
            None => None,
            Some(Json::String(s)) => Some(Some(s)),
            Some(_) => Some(None),
        },
    })
}

/// The plain value of a fragment: read as a call's arguments, so a bare `a, b` list reads too.
fn plain_value(source: &str) -> Result<Value, JsError> {
    let bump = Bump::new();
    let text = format!("v({source})");
    let root = parse_tree(&text, &bump).map_err(|e| error("K7SyntaxError", e.to_string()))?;
    let first = root.entries.first().ok_or_else(|| type_error("value"))?;
    Ok(k7x::edit::Tree::from_node(&first.value).to_value())
}

fn apply_one(doc: &mut Document, o: &Op, work: &Work) -> Result<(), JsError> {
    let text = doc.text().to_owned();
    if !work.spend(text.len()) {
        return Err(error("Budget", "the replay is over its work budget"));
    }
    let bump = Bump::new();
    let root =
        Node::Map(parse_tree(&text, &bump).map_err(|e| error("K7SyntaxError", e.to_string()))?);
    let cache = SegCache::new(work);
    let loc = locate(&cache, &root, &o.path);
    if loc.gone {
        return Err(error(
            "Error",
            format!("no list item at {}", o.path.join(".")),
        ));
    }
    if o.del {
        if loc.missing.is_none() {
            doc.delete_in(&loc.at, false).map_err(edit_error)?;
        }
        return Ok(());
    }
    let source = o.k7x.ok_or_else(|| type_error("k7x"))?;
    let Some(missing) = loc.missing else {
        return doc.replace_source(&loc.at, source).map_err(edit_error);
    };
    let last = o.path.len() - 1;
    let parent = root.at(&loc.at);
    if missing == last && matches!(parent, Some(Node::List { .. })) {
        let names = parent.and_then(|l| cache.of(last.checked_sub(1).map(|j| o.path[j]), l));
        let segs = names.as_ref().map_or(&[][..], |s| s.names.as_slice());
        // An anchor removed meanwhile puts the item last: the rule only has to be the same everywhere.
        let index = match o.after {
            Some(None) => 0,
            Some(Some(a)) => segs
                .iter()
                .position(|s| s == a)
                .map_or(segs.len(), |i| i + 1),
            None => segs.len(),
        };
        let value = plain_value(source)?;
        return doc
            .insert_in(&loc.at, index, &value, WriteOptions::default())
            .map_err(edit_error);
    }
    let mut at = loc.at;
    at.extend(
        o.path[missing..]
            .iter()
            .map(|s| PathSeg::Key((*s).to_owned())),
    );
    doc.set_in(&at, &plain_value(source)?, WriteOptions::default())
        .map_err(edit_error)
}

/// `text` with `ops` applied, in order; Err when the text does not read or an op has nothing to land on.
pub fn apply_k7x(text: &str, ops: &[Json]) -> Result<String, JsError> {
    apply_within(text, ops, &Work::unlimited())
}

/// `apply_k7x` held to a work budget: Err once the replay has cost more than it allows.
pub fn apply_within(text: &str, ops: &[Json], work: &Work) -> Result<String, JsError> {
    let mut parsed = Vec::with_capacity(ops.len());
    for o in ops {
        let op = op_of(o)?;
        if op.path.is_empty() {
            return match (op.set, op.k7x) {
                (true, Some(k)) => Ok(k.to_owned()),
                (true, None) => Err(type_error("k7x")),
                (false, _) => Ok(String::new()),
            };
        }
        parsed.push(op);
    }
    let mut doc = Document::new(text);
    if let Some(e) = doc.errors().first() {
        return Err(error("K7SyntaxError", e.to_string()));
    }
    for o in &parsed {
        apply_one(&mut doc, o, work)?;
    }
    Ok(doc.text().to_owned())
}

/// `op` with its `ops` replaced, the rest of it kept in place.
fn with_ops(op: &Json, ops: Vec<Json>) -> Json {
    let mut out = op.clone();
    if let Json::Object(m) = &mut out {
        m.insert("ops".into(), Json::Array(ops));
    }
    out
}

/// A k7x op against the room's `current` text: what still finds the value it replaced, what is set aside, the text after.
pub fn settle_k7x(current: Option<&str>, op: &Json) -> Result<Settled, JsError> {
    let Some(ops) = op.get("ops").and_then(Json::as_array) else {
        return Err(error("TypeError", "op.ops is not iterable"));
    };
    let bump = Bump::new();
    let read = current.and_then(|c| parse_tree(c, &bump).ok().map(|r| (c, Node::Map(r))));
    let Some((current, root)) = read else {
        let aside = ops
            .iter()
            .map(|o| (with_ops(op, vec![o.clone()]), None))
            .collect();
        return Ok(Settled {
            apply: None,
            aside,
            next: Ok(current.map(String::from)),
        });
    };
    let work = Work::unlimited();
    let cache = SegCache::new(&work);
    let mut keep = Vec::new();
    let mut aside = Vec::new();
    for o in ops {
        let was = was_of(o);
        if was == Was::Missing {
            keep.push(o.clone());
            continue;
        }
        let path = path_of(o)?;
        let now = value_at(&cache, current, &root, &path);
        let is_set = o.get("op").and_then(Json::as_str) == Some("set");
        // Already what the op makes it: nothing to apply, and nothing lost.
        let made = if is_set {
            o.get("k7x")
                .and_then(Json::as_str)
                .is_some_and(|k| now.as_deref() == Some(k))
        } else {
            now.is_none()
        };
        if made {
            continue;
        }
        let found = match was {
            Was::Null => now.is_none(),
            Was::Text(t) => now.as_deref() == Some(t),
            Was::Missing | Was::Other => false,
        };
        // An edit inside a list item someone removed has nothing to land on.
        if found && !gone_at(&cache, &root, &path) {
            keep.push(o.clone());
        } else {
            aside.push((with_ops(op, vec![o.clone()]), Some(now)));
        }
    }
    if keep.is_empty() {
        return Ok(Settled {
            apply: None,
            aside,
            next: Ok(Some(current.to_owned())),
        });
    }
    let apply = with_ops(op, keep.clone());
    let next = apply_k7x(current, &keep).map(Some).map_err(|_| ());
    Ok(Settled {
        apply: Some(apply),
        aside,
        next,
    })
}
