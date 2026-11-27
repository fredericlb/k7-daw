// ops/k7x.rs — the k7x.* operations: a file's data, its outline for the editor, a value printed, a batch of edits.
// A reader would get wrong what an offset is here: every span and change that crosses counts UTF-16 units, as the page does.
// A text that does not read is an answer (`syntax`), not an error: the editor asks that of every half-typed line.

use crate::error::{ErrorCode, WasmError};
use crate::ops::k7x_edit::{self, EditOp, Seg, to_value};
use crate::ops::write;
use k7x::edit::{PKind, PNode, Value, from_js, header_text, print_file};
use k7x::json::map_value_json;
use k7x::outline::outline as outline_of;
use k7x::{Bump, SyntaxError, parse_tree, parse_tree_lossless};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EditArgs {
    pub text: String,
    pub ops: Vec<EditOp>,
}

/// Comment lines a printed node gets above it, by its path from the printed value.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Lead {
    pub path: Vec<Seg>,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrintArgs {
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub value: serde_json::Value,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub header: Option<Vec<String>>,
    /// The paths printed on one line whatever their width.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub inline: Option<Vec<Vec<Seg>>>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub lead: Option<Vec<Lead>>,
}

/// Where a text stops reading: the reader's sentence, its offset in UTF-16 units, its 1-based line and column.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct K7xSyntax {
    pub message: String,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl K7xSyntax {
    pub fn of(text: &str, e: &SyntaxError) -> K7xSyntax {
        K7xSyntax {
            message: e.message.clone(),
            offset: text.get(..e.offset).map_or(0, |b| b.encode_utf16().count()),
            line: e.line,
            column: e.column,
        }
    }
}

/// What `k7x.data` and `k7x.outline` answer: the data, or the outline's keys and scalars, or where the text stops reading.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReadAnswer {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Record<string, unknown>"))]
    pub data: Option<()>,
    /// The outline's keys and scalars; its layout is the cells `k7_cells_ptr` points at.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Array<string | number | boolean | null>"))]
    pub values: Option<()>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub syntax: Option<K7xSyntax>,
}

/// The k7x family's requests: `{"op": "k7x.edit", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum K7xRequest {
    #[serde(rename = "k7x.data")]
    Data(TextArgs),
    #[serde(rename = "k7x.outline")]
    Outline(TextArgs),
    #[serde(rename = "k7x.print")]
    Print(PrintArgs),
    #[serde(rename = "k7x.edit")]
    Edit(EditArgs),
}

/// What the family keeps between calls: the arena a read builds its tree in, and the last outline's layout.
#[derive(Default)]
pub struct K7xState {
    pub arena: Bump,
    /// Valid until the next call: the ABI hands JS a view of it.
    pub cells: Vec<u32>,
}

/// The answer of a text that does not read.
fn syntax(text: &str, e: &SyntaxError, out: &mut Vec<u8>) -> Result<(), WasmError> {
    let answer = ReadAnswer {
        data: None,
        values: None,
        syntax: Some(K7xSyntax::of(text, e)),
    };
    write(out, &answer)
}

/// The data and the outline write their JSON by hand: a big file's answer is most of the call.
fn read(text: &str, state: &mut K7xState, outline: bool, out: &mut Vec<u8>) -> Result<(), WasmError> {
    state.cells.clear();
    state.arena.reset();
    let tree = match parse_tree_lossless(text, &state.arena) {
        Ok(Some(t)) => t,
        // A lone surrogate escape: the lossy tree, as the document reads it (rustEdit.md).
        Ok(None) => parse_tree(text, &state.arena).map_err(|e| WasmError::bad_request(e.to_string()))?,
        Err(e) => return syntax(text, &e, out),
    };
    let mut json = String::with_capacity(text.len() + 16);
    if outline {
        json.push_str("{\"values\":");
        outline_of(&tree, text, &mut state.cells, &mut json);
    } else {
        json.push_str("{\"data\":");
        map_value_json(&tree, &mut json);
    }
    json.push('}');
    out.extend_from_slice(json.as_bytes());
    Ok(())
}

fn print(a: &PrintArgs) -> Result<String, WasmError> {
    let value = to_value(&a.value);
    let inline: Vec<Vec<k7x::PathSeg>> = a.inline.iter().flatten().map(|p| k7x_edit::path(p)).collect();
    let one = |p: &[k7x::PathSeg], _: &Value| inline.iter().any(|q| q.as_slice() == p);
    let mut root = from_js(&value, Some(&one), &mut Vec::new());
    for l in a.lead.iter().flatten() {
        if let Some(n) = node_mut(&mut root, &k7x_edit::path(&l.path)) {
            n.lead.clone_from(&l.lines);
        }
    }
    let body = print_file(&root, &[], &[]).map_err(|e| bad_k7x(e.to_string()))?;
    let header = a.header.as_deref().unwrap_or_default();
    if header.is_empty() {
        return Ok(body);
    }
    Ok(header_text(header) + if body == "\n" { "" } else { &body })
}

/// The print node at a path of a printed value.
fn node_mut<'a>(mut n: &'a mut PNode, path: &[k7x::PathSeg]) -> Option<&'a mut PNode> {
    for seg in path {
        n = match (&mut n.kind, seg) {
            (PKind::Map(es), k7x::PathSeg::Key(k)) => &mut es.iter_mut().find(|e| e.key == *k)?.value,
            (PKind::List(items), k7x::PathSeg::Index(i)) => items.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(n)
}

pub(crate) fn bad_k7x(message: String) -> WasmError {
    WasmError::new(ErrorCode::BadK7x, message)
}

pub fn dispatch(state: &mut K7xState, request: K7xRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        K7xRequest::Data(a) => read(&a.text, state, false, out),
        K7xRequest::Outline(a) => read(&a.text, state, true, out),
        K7xRequest::Print(a) => write(out, &print(&a)?),
        K7xRequest::Edit(a) => write(out, &k7x_edit::run(&a.text, &a.ops)),
    }
}
