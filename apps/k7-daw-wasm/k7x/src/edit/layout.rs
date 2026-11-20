// layout.rs — the questions an edit asks of the text around a span: where its line starts, how deep it is indented, what trails it.
// A reader would get wrong that these read the raw text, not the tree: they see comments the tree drops. Offsets are bytes.

use super::tree::Tree;
use super::value::{is_js_space, u16len};
use crate::SyntaxError;
use std::fmt;
use std::rc::Rc;

/// What an edit fails with: the reader's error on an unreadable text, or the editor's own sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    Syntax(SyntaxError),
    Edit(String),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Syntax(e) => e.fmt(f),
            EditError::Edit(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for EditError {}

pub type Res<T> = Result<T, EditError>;

pub(crate) fn fail<T>(message: impl Into<String>) -> Res<T> {
    Err(EditError::Edit(message.into()))
}

/// What the editing modules need of a document: its text, its tree, and the one way to change it.
pub trait Host {
    fn text(&self) -> Rc<str>;
    fn root(&self) -> Res<Rc<Tree>>;
    fn splice(&mut self, start: usize, end: usize, insert: &str) -> Res<()>;
}

pub const BOM: &str = "\u{feff}";

#[must_use]
pub fn pad(n: usize) -> String {
    " ".repeat(n)
}

/// Where the line holding `pos` starts; a text opening with a newline and `pos` 0 gives 1, as `lastIndexOf("\n", -1)` does.
#[must_use]
pub fn line_start(t: &str, pos: usize) -> usize {
    if pos == 0 {
        return usize::from(t.starts_with('\n'));
    }
    t[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// The columns before `pos` on its line, in UTF-16 units.
#[must_use]
pub fn column(t: &str, pos: usize) -> usize {
    t.get(line_start(t, pos)..pos).map_or(0, u16len)
}

#[must_use]
pub fn head_blank(t: &str, pos: usize) -> bool {
    t.get(line_start(t, pos)..pos)
        .is_some_and(|s| s.bytes().all(|b| b == b' ' || b == b'\t'))
}

#[must_use]
pub fn indent_of(t: &str, pos: usize) -> usize {
    t.get(line_start(t, pos)..).map_or(0, |s| {
        s.bytes().take_while(|&b| b == b' ' || b == b'\t').count()
    })
}

/// The file's header: the `//` lines it starts with, each without `//` and one space, and the offset past the last one's newline.
#[must_use]
pub fn read_header(t: &str) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut pos = if t.starts_with(BOM) { BOM.len() } else { 0 };
    while t[pos..].starts_with("//") {
        let nl = t[pos..].find('\n').map(|i| pos + i);
        let end = nl.unwrap_or(t.len());
        let line = &t[pos + 2..end];
        let line = line.strip_suffix('\r').unwrap_or(line);
        lines.push(line.strip_prefix(' ').unwrap_or(line).to_owned());
        pos = nl.map_or(t.len(), |i| i + 1);
    }
    (lines, pos)
}

/// The offset of the newline ending the line after `pos`, when only blanks, a comma (if allowed) and a comment follow; else None.
#[must_use]
pub fn tail_end(t: &str, pos: usize, comma: bool) -> Option<usize> {
    let b = t.as_bytes();
    let mut i = pos;
    let skip = |i: &mut usize, cr: bool| {
        while *i < b.len() && (b[*i] == b' ' || b[*i] == b'\t' || (cr && b[*i] == b'\r')) {
            *i += 1;
        }
    };
    if comma {
        skip(&mut i, false);
        if b.get(i) == Some(&b',') {
            i += 1;
        }
    }
    skip(&mut i, true);
    if t[i..].starts_with("//") {
        i = t[i..].find('\n').map_or(t.len(), |k| i + k);
    }
    (i >= b.len() || b[i] == b'\n').then_some(i)
}

/// The offset of the comma after `pos` past whitespace and comments, or None when something else comes first.
#[must_use]
pub fn comma_after(t: &str, pos: usize) -> Option<usize> {
    let mut i = pos;
    loop {
        let rest = &t[i..];
        match rest.chars().next() {
            Some(c) if is_js_space(c) => i += c.len_utf8(),
            Some('/') if rest.starts_with("//") => i = rest.find('\n').map_or(t.len(), |k| i + k),
            Some('/') if rest.starts_with("/*") => match rest[2..].find("*/") {
                Some(k) => i += 2 + k + 2,
                None => break,
            },
            _ => break,
        }
    }
    (t.as_bytes().get(i) == Some(&b',')).then_some(i)
}
