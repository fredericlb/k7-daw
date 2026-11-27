// outline.rs — a tree as an editor reads it, preorder: its layout as numbers, its keys and scalars as one JSON array (docs/outline.md).
// A reader would get wrong why the numbers are not JSON: parsed back by JSON.parse they cost more than the whole JS parse did (docs/outline.md).

use crate::ast::{Entry, ListForm, MapForm, MapNode, Node};
use crate::json::{scalar_json, string_json, utf16_offsets};

/// A node's first cell: what follows it is the node's own layout.
pub const SCALAR: u32 = 0;
pub const BRACKETS: u32 = 1;
pub const ARGS: u32 = 2;
pub const ROOT: u32 = 3;
pub const CHAIN: u32 = 4;
pub const OBJECT: u32 = 5;
/// The dot cell of an entry that has none.
pub const NO_DOT: u32 = u32::MAX;

struct Out<'a> {
    u: Vec<usize>,
    cells: &'a mut Vec<u32>,
    values: &'a mut String,
    any: bool,
}

impl Out<'_> {
    fn num(&mut self, n: usize) {
        self.cells.push(u32::try_from(n).unwrap_or(u32::MAX));
    }

    /// A byte offset, as UTF-16 units.
    fn at(&mut self, byte: usize) {
        self.num(self.u[byte]);
    }

    fn value_sep(&mut self) {
        if self.any {
            self.values.push(',');
        }
        self.any = true;
    }
}

/// The outline of a whole file: `cells` its layout, `values` the JSON array of its keys and scalars in the order the cells meet them.
pub fn outline(root: &MapNode, text: &str, cells: &mut Vec<u32>, values: &mut String) {
    cells.clear();
    values.push('[');
    let mut o = Out {
        u: utf16_offsets(text),
        cells,
        values,
        any: false,
    };
    map(root, &mut o);
    o.values.push(']');
}

fn node(n: &Node, o: &mut Out) {
    match n {
        Node::Scalar { value, start, end } => {
            o.num(SCALAR as usize);
            o.at(*start);
            o.at(*end);
            o.value_sep();
            scalar_json(value, o.values);
        }
        Node::List {
            items,
            form,
            start,
            end,
        } => {
            o.num(if *form == ListForm::Args {
                ARGS
            } else {
                BRACKETS
            } as usize);
            o.at(*start);
            o.at(*end);
            o.num(items.len());
            for i in *items {
                node(i, o);
            }
        }
        Node::Map(m) => map(m, o),
    }
}

fn map(m: &MapNode, o: &mut Out) {
    let tag = match m.form {
        MapForm::Root => ROOT,
        MapForm::Chain => CHAIN,
        MapForm::Object => OBJECT,
    };
    o.num(tag as usize);
    o.at(m.start);
    o.at(m.end);
    o.num(m.entries.len());
    for e in m.entries {
        entry(e, o);
    }
    o.num(m.groups.len());
    for (a, b) in m.groups {
        o.num(*a);
        o.num(*b);
    }
}

fn entry(e: &Entry, o: &mut Out) {
    o.value_sep();
    string_json(e.key, o.values);
    o.at(e.key_start);
    o.at(e.key_end);
    o.at(e.start);
    o.at(e.end);
    match e.dot {
        Some(d) => o.at(d),
        None => o.cells.push(NO_DOT),
    }
    node(&e.value, o);
}
