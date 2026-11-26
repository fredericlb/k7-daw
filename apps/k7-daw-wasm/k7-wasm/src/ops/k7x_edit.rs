// ops/k7x_edit.rs — `k7x.edit`: a gesture's edits run in order on one document, answering the text and its splices.
// A reader would get wrong what a failed op leaves: the edits before it stay applied, as a writer calling them one by one saw.

use crate::ops::k7x::K7xSyntax;
use k7x::PathSeg;
use k7x::edit::{Document, EditError, Value, WriteOptions};
use serde::{Deserialize, Serialize};

/// A path segment: a map key, or a list index (a key of digits names an index too).
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum Seg {
    Index(usize),
    Key(String),
}

pub fn path(p: &[Seg]) -> Vec<PathSeg> {
    p.iter()
        .map(|s| match s {
            Seg::Index(i) => PathSeg::Index(*i),
            Seg::Key(k) => PathSeg::Key(k.clone()),
        })
        .collect()
}

/// One edit, named as the document's method it runs.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EditOp {
    SetIn {
        path: Vec<Seg>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: serde_json::Value,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        inline: Option<bool>,
    },
    SetSourceIn {
        path: Vec<Seg>,
        source: String,
    },
    AddIn {
        path: Vec<Seg>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: serde_json::Value,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        inline: Option<bool>,
    },
    ReplaceSource {
        path: Vec<Seg>,
        source: String,
    },
    InsertIn {
        path: Vec<Seg>,
        index: usize,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: serde_json::Value,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        inline: Option<bool>,
    },
    InsertSourceIn {
        path: Vec<Seg>,
        index: usize,
        source: String,
    },
    MoveIn {
        path: Vec<Seg>,
        from: usize,
        to: usize,
    },
    RenameKey {
        path: Vec<Seg>,
        key: String,
    },
    ApplyIn {
        path: Vec<Seg>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: serde_json::Value,
    },
    DeleteIn {
        path: Vec<Seg>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        prune: Option<bool>,
    },
    SetHeader {
        lines: Vec<String>,
    },
}

/// One splice, in UTF-16 units of the text as it was just before it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct K7Change {
    pub start: usize,
    pub end: usize,
    pub insert: String,
}

/// Why an op failed: its index in the batch, the editor's sentence, and the reader's position when the text did not read.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EditFailure {
    pub op: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub syntax: Option<K7xSyntax>,
}

/// What `k7x.edit` answers: the text after the ops that ran, their splices, each op's own answer, and the failure that stopped it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EditAnswer {
    pub text: String,
    pub changes: Vec<K7Change>,
    /// `deleteIn`'s boolean, null for every other op.
    pub results: Vec<Option<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<EditFailure>,
}

/// A JSON value as k7x data, its maps ordered as the JS object it came from.
pub fn to_value(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(to_value).collect()),
        serde_json::Value::Object(m) => Value::object(m.iter().map(|(k, v)| (k.clone(), to_value(v))).collect()),
    }
}

fn opts(inline: Option<bool>) -> WriteOptions {
    WriteOptions {
        inline: inline.unwrap_or(false),
    }
}

fn exec(doc: &mut Document, op: &EditOp) -> Result<Option<bool>, EditError> {
    match op {
        EditOp::SetIn { path: p, value, inline } => doc.set_in(&path(p), &to_value(value), opts(*inline))?,
        EditOp::SetSourceIn { path: p, source } => doc.set_source_in(&path(p), source)?,
        EditOp::AddIn { path: p, value, inline } => doc.add_in(&path(p), &to_value(value), opts(*inline))?,
        EditOp::ReplaceSource { path: p, source } => doc.replace_source(&path(p), source)?,
        EditOp::InsertIn {
            path: p,
            index,
            value,
            inline,
        } => doc.insert_in(&path(p), *index, &to_value(value), opts(*inline))?,
        EditOp::InsertSourceIn { path: p, index, source } => doc.insert_source_in(&path(p), *index, source)?,
        EditOp::MoveIn { path: p, from, to } => doc.move_in(&path(p), *from, *to)?,
        EditOp::RenameKey { path: p, key } => doc.rename_key(&path(p), key)?,
        EditOp::ApplyIn { path: p, value } => doc.apply_in(&path(p), &to_value(value))?,
        EditOp::DeleteIn { path: p, prune } => return doc.delete_in(&path(p), prune.unwrap_or(false)).map(Some),
        EditOp::SetHeader { lines } => doc.set_header(lines)?,
    }
    Ok(None)
}

/// Runs `ops` in order on `text`, stopping at the first that fails.
pub fn run(text: &str, ops: &[EditOp]) -> EditAnswer {
    let mut doc = Document::new(text);
    let mut results = Vec::with_capacity(ops.len());
    let mut error = None;
    for (i, op) in ops.iter().enumerate() {
        match exec(&mut doc, op) {
            Ok(r) => results.push(r),
            Err(e) => {
                let syntax = match &e {
                    EditError::Syntax(s) => Some(K7xSyntax::of(doc.text(), s)),
                    EditError::Edit(_) => None,
                };
                error = Some(EditFailure {
                    op: i,
                    message: e.to_string(),
                    syntax,
                });
                break;
            }
        }
    }
    EditAnswer {
        text: doc.text().to_owned(),
        changes: doc
            .changes()
            .iter()
            .map(|c| K7Change {
                start: c.start16,
                end: c.end16,
                insert: c.insert.clone(),
            })
            .collect(),
        results,
        error,
    }
}
