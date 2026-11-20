// tree.rs — the parsed tree as an owned value, so a document can keep one tree while its text is spliced and parsed again.
// A reader would get wrong what a span is: a BYTE offset into the text the tree was parsed from, as in ast.rs.

use super::value::Value;
use crate::ast::{self, ListForm, MapForm, PathSeg};

#[derive(Debug, Clone, PartialEq)]
pub enum Tree {
    Scalar {
        value: Value,
        start: usize,
        end: usize,
    },
    List {
        items: Vec<Tree>,
        form: ListForm,
        start: usize,
        end: usize,
    },
    Map(MapTree),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapTree {
    pub entries: Vec<EntryTree>,
    pub form: MapForm,
    pub groups: Vec<(usize, usize)>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryTree {
    pub key: String,
    pub value: Tree,
    pub start: usize,
    pub end: usize,
    pub dot: Option<usize>,
    pub key_start: usize,
    pub key_end: usize,
}

impl Tree {
    #[must_use]
    pub fn from_node(n: &ast::Node) -> Tree {
        match n {
            ast::Node::Scalar { value, start, end } => Tree::Scalar {
                value: match value {
                    ast::Scalar::Str(s) => Value::Str((*s).to_owned()),
                    ast::Scalar::Num(x) => Value::Num(*x),
                    ast::Scalar::Bool(b) => Value::Bool(*b),
                    ast::Scalar::Null => Value::Null,
                },
                start: *start,
                end: *end,
            },
            ast::Node::List {
                items,
                form,
                start,
                end,
            } => Tree::List {
                items: items.iter().map(Tree::from_node).collect(),
                form: *form,
                start: *start,
                end: *end,
            },
            ast::Node::Map(m) => Tree::Map(MapTree::from_node(m)),
        }
    }

    #[must_use]
    pub fn start(&self) -> usize {
        match self {
            Tree::Scalar { start, .. } | Tree::List { start, .. } => *start,
            Tree::Map(m) => m.start,
        }
    }

    #[must_use]
    pub fn end(&self) -> usize {
        match self {
            Tree::Scalar { end, .. } | Tree::List { end, .. } => *end,
            Tree::Map(m) => m.end,
        }
    }

    /// The plain value the node stands for (`toJS`).
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Tree::Scalar { value, .. } => value.clone(),
            Tree::List { items, .. } => Value::List(items.iter().map(Tree::to_value).collect()),
            Tree::Map(m) => m.to_value(),
        }
    }

    /// A node's child at one path segment: a map entry by key, a list item by index or a string of digits.
    #[must_use]
    pub fn child_at(&self, seg: &PathSeg) -> Option<&Tree> {
        match (self, seg) {
            (Tree::Map(m), PathSeg::Key(k)) => {
                m.entries.iter().find(|e| e.key == *k).map(|e| &e.value)
            }
            (Tree::List { items, .. }, s) => items.get(index_of(s)?),
            _ => None,
        }
    }

    /// The node at a path, or None.
    #[must_use]
    pub fn node_at(&self, path: &[PathSeg]) -> Option<&Tree> {
        path.iter().try_fold(self, |n, seg| n.child_at(seg))
    }
}

impl MapTree {
    #[must_use]
    pub fn from_node(m: &ast::MapNode) -> MapTree {
        MapTree {
            entries: m
                .entries
                .iter()
                .map(|e| EntryTree {
                    key: e.key.to_owned(),
                    value: Tree::from_node(&e.value),
                    start: e.start,
                    end: e.end,
                    dot: e.dot,
                    key_start: e.key_start,
                    key_end: e.key_end,
                })
                .collect(),
            form: m.form,
            groups: m.groups.to_vec(),
            start: m.start,
            end: m.end,
        }
    }

    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(
            self.entries
                .iter()
                .map(|e| (e.key.clone(), e.value.to_value()))
                .collect(),
        )
    }
}

/// A list index from a path segment: an index, or a string of digits ("2" is 2).
#[must_use]
pub fn index_of(seg: &PathSeg) -> Option<usize> {
    match seg {
        PathSeg::Index(i) => Some(*i),
        PathSeg::Key(k) => (k == "0"
            || (!k.starts_with('0') && !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())))
        .then(|| k.parse().ok())
        .flatten(),
    }
}
