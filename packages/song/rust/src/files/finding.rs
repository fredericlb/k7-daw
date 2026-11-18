// files/finding.rs — a finding placed on its file's own text, and where a key path is written in a k7x tree.
// A reader would get wrong the column: UTF-16 units, 1-based, because that is what the editor counts.

use k7x::{MapNode, Node, PathSeg, SyntaxError};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One thing wrong with one file, placed on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Finding {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: Severity,
}

/// The 1-based line and the 1-based UTF-16 column of a byte offset in `text`.
pub fn line_col(text: &str, offset: usize) -> (u32, u32) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let tail = before.rsplit('\n').next().unwrap_or("");
    let col = tail.encode_utf16().count() + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(col).unwrap_or(u32::MAX),
    )
}

impl Finding {
    /// An error at a byte offset of `text`.
    pub fn at(file: &str, text: &str, offset: usize, message: impl Into<String>) -> Finding {
        let (line, col) = line_col(text, offset);
        Finding {
            file: file.to_owned(),
            line,
            col,
            message: message.into(),
            severity: Severity::Error,
        }
    }

    /// A syntax error at its own position, whose column the parser already counts in UTF-16 units.
    pub fn syntax(file: &str, e: &SyntaxError) -> Finding {
        let at = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Finding {
            file: file.to_owned(),
            line: at(e.line),
            col: at(e.column),
            message: e.message.clone(),
            severity: Severity::Error,
        }
    }

    /// The same finding named by another file.
    pub fn in_file(&self, file: &str) -> Finding {
        Finding {
            file: file.to_owned(),
            ..self.clone()
        }
    }
}

fn position(root: &Node, path: &[PathSeg]) -> Option<usize> {
    let (last, head) = path.split_last()?;
    if let (Some(Node::Map(m)), PathSeg::Key(k)) = (root.at(head), last)
        && let Some(e) = m.entries.iter().find(|e| e.key == k)
    {
        return Some(e.key_start);
    }
    root.at(path).map(|n| n.start())
}

/// The byte offset of a path: the deepest part of it that exists, at its key when it has one.
pub fn locate(root: &MapNode, path: &[PathSeg]) -> Option<usize> {
    let root = Node::Map(*root);
    (1..=path.len()).rev().find_map(|n| position(&root, path.get(..n)?))
}

/// A path of map keys from plain names.
pub fn keys(names: &[&str]) -> Vec<PathSeg> {
    names.iter().map(|n| PathSeg::Key((*n).to_owned())).collect()
}
