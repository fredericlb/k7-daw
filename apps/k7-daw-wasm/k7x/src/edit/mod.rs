// edit — the k7x document editor and printer: every write a splice of the value it changes (docs/rustEdit.md).
// A reader would get wrong what an offset is: a BYTE of the text; only widths and columns count UTF-16 units, as the page does.

mod apply;
mod document;
mod layout;
mod print;
mod remove;
mod scope;
mod sugared;
mod tree;
mod value;
mod write;

pub use crate::ast::PathSeg;
pub use apply::{PathEditor, apply_data};
pub use document::{Change, Document, WriteOptions, parse};
pub use layout::{BOM, EditError, Host, Res, read_header};
pub use print::{
    Ctx, InlineFn, Out, PEntry, PKind, PNode, StringifyOptions, from_js, header_text, print,
    print_file, print_key, stringify,
};
pub use scope::Scope;
pub use sugared::{Sugar, SugarPart, apply_sugared};
pub use tree::{EntryTree, MapTree, Tree};
pub use value::{Value, is_js_space, js_number, js_trim, json_text, same};
