//! refs.rs — the `@workspace/…` module paths a text names, as `copyProject.ts`' `WORKSPACE_REF` matches them: a
//! matcher of a dozen lines in place of a regex, held to the regex by the vectors (1652).
//! What a reader gets wrong: a ref is cleaned (`.k7x`, then trailing `/`) and one climbing with `..` is dropped.

use std::collections::{HashSet, VecDeque};

use crate::paths::extname;

/// The marker a ref starts at.
const PREFIX: &str = "@workspace/";
/// The text a module reference can be written in; audio and caches are never read.
const TEXT: [&str; 7] = [".k7x", ".yaml", ".yml", ".json", ".ts", ".js", ".md"];

/// Whether a file of this name is scanned for refs: its `extname` exactly, case kept.
pub fn is_text_name(name: &str) -> bool {
    TEXT.contains(&extname(name).as_str())
}

/// The first character of a ref's path: `[A-Za-z0-9_-]`.
fn head(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Every character after it: `[A-Za-z0-9_.\-/]`.
fn rest(b: u8) -> bool {
    head(b) || b == b'.' || b == b'/'
}

/// `text.matchAll(WORKSPACE_REF)`: each match's captured path, uncleaned, left to right without overlap.
pub fn raw_matches(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find(PREFIX).map(|i| i + from) {
        let start = i + PREFIX.len();
        if start < b.len() && head(b[start]) {
            let mut end = start + 1;
            while end < b.len() && rest(b[end]) {
                end += 1;
            }
            out.push(&text[start..end]);
            from = end;
        } else {
            from = i + 1;
        }
    }
    out
}

/// A matched path cleaned as `refsIn` cleans it: `.k7x` off the end, then trailing `/`; none for a `..` segment.
pub fn clean(raw: &str) -> Option<String> {
    let path = raw
        .strip_suffix(".k7x")
        .unwrap_or(raw)
        .trim_end_matches('/');
    (!path.split('/').any(|s| s == "..")).then(|| path.to_string())
}

/// A set that keeps its insertion order and gives its first back: the TypeScript's `Set` of module paths still to carry.
#[derive(Default)]
pub struct Todo {
    queue: VecDeque<String>,
    held: HashSet<String>,
}

impl Todo {
    pub fn add(&mut self, path: String) {
        if self.held.insert(path.clone()) {
            self.queue.push_back(path);
        }
    }

    /// `const [path] = todo; todo.delete(path)`.
    pub fn take(&mut self) -> Option<String> {
        let p = self.queue.pop_front()?;
        self.held.remove(&p);
        Some(p)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn items(&self) -> Vec<String> {
        self.queue.iter().cloned().collect()
    }
}

/// `refsIn`: the cleaned module paths `text` names, added in the order found.
pub fn refs_in(text: &str, into: &mut Todo) {
    for raw in raw_matches(text) {
        if let Some(p) = clean(raw) {
            into.add(p);
        }
    }
}
