// document.rs — a k7x file as a document whose values are set, added, moved and deleted by key path, every other byte kept.
// The contract every writer relies on: a knob drag is a one-line diff and a comment survives every edit (write.rs, remove.rs).

use super::apply::{PathEditor, apply_data};
use super::layout::{BOM, EditError, Host, Res, fail, head_blank, read_header, tail_end};
use super::print::{PNode, StringifyOptions, from_js, header_text, print_key, stringify};
use super::remove::{prune, remove_at};
use super::scope::Scope;
use super::tree::{MapTree, Tree, index_of};
use super::value::{Value, js_trim_end, path_text, u16len};
use super::write::{holds, insert_at, write_at};
use crate::ast::{ListForm, PathSeg};
use crate::{Bump, SyntaxError, parse_tree, parse_tree_lossless, parse_value_end};
use std::rc::Rc;

/// How a written value is laid out: `inline` keeps it on one line whatever its width.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteOptions {
    pub inline: bool,
}

/// One splice: bytes of the text just before it, and the same span in UTF-16 units for an editor that counts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub start: usize,
    pub end: usize,
    pub insert: String,
    pub start16: usize,
    pub end16: usize,
}

fn leaf_of(value: &Value, opts: WriteOptions) -> PNode {
    let mut node = from_js(value, None, &mut Vec::new());
    node.inline |= opts.inline;
    node
}

fn raw_leaf(source: &str) -> Res<PNode> {
    let bump = Bump::new();
    let end = parse_value_end(source, &bump).map_err(EditError::Syntax)?;
    let last_line = source[end..].rsplit('\n').next().unwrap_or("");
    if last_line.contains("//") {
        return fail("k7x: source ending in a // comment would swallow what follows it");
    }
    Ok(PNode::raw(source))
}

pub struct Document {
    text: Rc<str>,
    contents: Option<Rc<Tree>>,
    errors: Vec<SyntaxError>,
    lossy: bool,
    changes: Vec<Change>,
}

impl Document {
    #[must_use]
    pub fn new(text: &str) -> Document {
        let mut d = Document {
            text: text.into(),
            contents: None,
            errors: vec![],
            lossy: false,
            changes: vec![],
        };
        d.reparse();
        d
    }

    fn reparse(&mut self) {
        let bump = Bump::new();
        let text = Rc::clone(&self.text);
        let tree = match parse_tree_lossless(&text, &bump) {
            Ok(Some(t)) => Ok((t, false)),
            Ok(None) => parse_tree(&text, &bump).map(|t| (t, true)),
            Err(e) => Err(e),
        };
        match tree {
            Ok((t, lossy)) => {
                self.contents = Some(Rc::new(Tree::Map(MapTree::from_node(&t))));
                self.errors = vec![];
                self.lossy = lossy;
            }
            Err(e) => {
                self.contents = None;
                self.errors = vec![e];
            }
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// Whether the text escapes a lone surrogate, which a Rust string reads as U+FFFD where JS keeps it.
    #[must_use]
    pub fn lossy(&self) -> bool {
        self.lossy
    }

    /// Every splice applied since construction, in order.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    fn tree(&self) -> Res<Rc<Tree>> {
        self.contents
            .clone()
            .ok_or_else(|| EditError::Syntax(self.errors[0].clone()))
    }

    /// The plain value of the file.
    pub fn to_js(&self) -> Res<Value> {
        Ok(self.tree()?.to_value())
    }

    /// Runs `f` on the node at a path.
    pub fn node_at<R>(&self, path: &[PathSeg], f: impl FnOnce(Option<&Tree>) -> R) -> Res<R> {
        let root = self.tree()?;
        Ok(f(root.node_at(path)))
    }

    pub fn get_in(&self, path: &[PathSeg]) -> Res<Option<Value>> {
        self.node_at(path, |n| n.map(Tree::to_value))
    }

    pub fn has_in(&self, path: &[PathSeg]) -> Res<bool> {
        self.node_at(path, |n| n.is_some())
    }

    /// The exact source of the node at a path; an empty call's flag, `key()`, has none and gives "".
    pub fn get_source(&self, path: &[PathSeg]) -> Res<Option<String>> {
        self.node_at(path, |n| {
            n.map(|n| self.text[n.start()..n.end()].to_owned())
        })
    }

    /// Where a path is written: a map entry's key, else the node's first character; line and column, 1-based, UTF-16.
    pub fn position_of(&self, path: &[PathSeg]) -> Res<Option<(usize, usize)>> {
        let root = self.tree()?;
        let entry = match (
            path.split_last(),
            path.split_last().and_then(|(_, p)| root.node_at(p)),
        ) {
            (Some((PathSeg::Key(k), _)), Some(Tree::Map(m))) => {
                m.entries.iter().find(|e| e.key == *k)
            }
            _ => None,
        };
        let at = entry
            .map(|e| e.key_start)
            .or_else(|| root.node_at(path).map(Tree::start));
        Ok(at.map(|a| line_col(&self.text, a)))
    }

    /// A view whose every path is relative to `base`.
    pub fn at(&mut self, base: &[PathSeg]) -> Scope<'_> {
        Scope::new(self, base.to_vec())
    }

    /// Sets the value at a path, creating the maps on the way; a value equal to the one there changes nothing.
    pub fn set_in(&mut self, path: &[PathSeg], value: &Value, opts: WriteOptions) -> Res<()> {
        if path.is_empty() {
            let text = stringify(value, &StringifyOptions::default())?;
            return self.splice(0, self.text.len(), &text);
        }
        write_at(self, path, &leaf_of(value, opts), &|n, _| holds(n, value))
    }

    /// Sets (or creates) the value at a path as k7x source, which must read as one value.
    pub fn set_source_in(&mut self, path: &[PathSeg], source: &str) -> Res<()> {
        if path.is_empty() {
            let bump = Bump::new();
            parse_tree(source, &bump).map_err(EditError::Syntax)?;
            return self.splice(0, self.text.len(), source);
        }
        write_at(self, path, &raw_leaf(source)?, &|n, t| {
            &t[n.start()..n.end()] == source
        })
    }

    /// Appends to the list at a path, creating it when it is missing.
    pub fn add_in(&mut self, path: &[PathSeg], value: &Value, opts: WriteOptions) -> Res<()> {
        let len = self.node_at(path, |n| match n {
            None => Ok(None),
            Some(Tree::List { items, .. }) => Ok(Some(items.len())),
            Some(_) => fail(format!("k7x: {} is not a list", path_text(path))),
        })??;
        match len {
            None => self.set_in(path, &Value::List(vec![value.clone()]), opts),
            Some(n) => {
                let mut p = path.to_vec();
                p.push(PathSeg::Index(n));
                self.set_in(&p, value, opts)
            }
        }
    }

    /// Replaces the value at a path with `source` as written; fails when nothing is there.
    pub fn replace_source(&mut self, path: &[PathSeg], source: &str) -> Res<()> {
        let span = self.node_at(path, |n| n.map(|n| (n.start(), n.end())))?;
        match span {
            Some((s, e)) if !path.is_empty() => self.splice(s, e, source),
            _ => fail(format!("k7x: no value at {} to replace", path_text(path))),
        }
    }

    /// Inserts into the list at a path before item `index`, on a line of its own when that item has one.
    pub fn insert_in(
        &mut self,
        path: &[PathSeg],
        index: usize,
        value: &Value,
        opts: WriteOptions,
    ) -> Res<()> {
        insert_at(self, path, index, &leaf_of(value, opts))
    }

    /// Inserts k7x source into the list at a path before item `index`; fails when it does not read as one value.
    pub fn insert_source_in(&mut self, path: &[PathSeg], index: usize, source: &str) -> Res<()> {
        insert_at(self, path, index, &raw_leaf(source)?)
    }

    /// Moves a list item so it ends up at index `to`, its source kept as written.
    pub fn move_in(&mut self, path: &[PathSeg], from: usize, to: usize) -> Res<()> {
        let root = self.tree()?;
        let Some(Tree::List {
            items,
            form,
            start,
            end,
        }) = root.node_at(path)
        else {
            return fail(format!("k7x: {} is not a list", path_text(path)));
        };
        let n = items.len();
        if from >= n || to >= n {
            return fail(format!("k7x: no move from {from} to {to} in {n} items"));
        }
        if from == to {
            return Ok(());
        }
        let mut sources: Vec<String> = items
            .iter()
            .map(|i| self.text[i.start()..i.end()].to_owned())
            .collect();
        if *form == ListForm::Args {
            let item = sources.remove(from);
            sources.insert(to, item);
            return self.splice(*start, *end, &sources.join(", "));
        }
        let note = self.line_comment(&items[from]);
        let mut at = path.to_vec();
        at.push(PathSeg::Index(from));
        self.delete_in(&at, false)?;
        self.insert_source_in(path, to, &sources[from])?;
        let moved_end = self.node_at(&[path, &[PathSeg::Index(to)]].concat(), |n| {
            n.map(Tree::end)
        })?;
        let moved = self.node_at(&[path, &[PathSeg::Index(to)]].concat(), |n| {
            n.map(|n| self.line_comment(n))
        })?;
        if let (Some(note), Some(end), Some(Some(c))) =
            (note.filter(|n| !n.is_empty()), moved_end, moved)
            && c.is_empty()
        {
            let t = Rc::clone(&self.text);
            let blanks = t[end..]
                .bytes()
                .take_while(|&b| b == b' ' || b == b'\t')
                .count();
            let comma = usize::from(t.as_bytes().get(end + blanks) == Some(&b','));
            let pos = end + blanks + comma;
            self.splice(pos, pos, &format!(" {note}"))?;
        }
        Ok(())
    }

    /// The `//` comment ending an item's line when the item has that line to itself: "" for none, None when it shares it.
    fn line_comment(&self, item: &Tree) -> Option<String> {
        let t = &self.text;
        let tail = tail_end(t, item.end(), true)?;
        if !head_blank(t, item.start()) {
            return None;
        }
        let between = &t[item.end()..tail];
        Some(
            between
                .find("//")
                .map_or(String::new(), |i| js_trim_end(&between[i..]).to_owned()),
        )
    }

    /// Renames the entry at a path, its value, comments and place kept; fails when the map already has `new_key`.
    pub fn rename_key(&mut self, path: &[PathSeg], new_key: &str) -> Res<()> {
        let root = self.tree()?;
        let found = match path.split_last() {
            Some((PathSeg::Key(k), parent)) => match root.node_at(parent) {
                Some(Tree::Map(m)) => m.entries.iter().find(|e| e.key == *k).map(|e| (m, e)),
                _ => None,
            },
            _ => None,
        };
        let Some((m, e)) = found else {
            return fail(format!("k7x: no entry at {} to rename", path_text(path)));
        };
        if e.key == new_key {
            return Ok(());
        }
        if m.entries.iter().any(|x| x.key == new_key) {
            return fail(format!("k7x: {new_key} is already a key there"));
        }
        self.splice(e.key_start, e.key_end, &print_key(new_key))
    }

    /// Makes the data at a path equal `value` with the fewest edits; [] is the whole file.
    pub fn apply_in(&mut self, path: &[PathSeg], value: &Value) -> Res<()> {
        match self.get_in(path)? {
            Some(before) => apply_data(self, &before, value, path),
            None => self.set_in(path, value, WriteOptions::default()),
        }
    }

    /// Removes the entry or item at a path; `prune` also removes the maps that leaves empty, up to a list or the root.
    pub fn delete_in(&mut self, path: &[PathSeg], prune_empty: bool) -> Res<bool> {
        let Some((seg, parent_path)) = path.split_last() else {
            return Ok(false);
        };
        let root = self.tree()?;
        let Some(parent) = root.node_at(parent_path) else {
            return Ok(false);
        };
        let at = match (parent, index_of(seg)) {
            (Tree::List { .. }, Some(i)) => PathSeg::Index(i),
            _ => seg.clone(),
        };
        if !remove_at(self, parent, &at)? {
            return Ok(false);
        }
        if prune_empty {
            prune(self, path)?;
        }
        Ok(true)
    }

    /// The `//` lines the file starts with, each without `//` and one space.
    #[must_use]
    pub fn header(&self) -> Vec<String> {
        read_header(&self.text).0
    }

    /// Replaces the file's header with these lines; an empty slice removes it.
    pub fn set_header(&mut self, lines: &[String]) -> Res<()> {
        let (old, end) = read_header(&self.text);
        let top = header_text(lines);
        if header_text(&old) == top {
            return Ok(());
        }
        let start = if self.text.starts_with(BOM) {
            BOM.len()
        } else {
            0
        };
        let nl = end == self.text.len() && end > start && !self.text.ends_with('\n');
        let top = if nl { &top[..top.len() - 1] } else { &top };
        self.splice(start, end, top)
    }
}

/// The 1-based line and column of an offset, the column in UTF-16 units.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = before
        .rfind('\n')
        .map_or(u16len(before), |i| u16len(&before[i + 1..]))
        + 1;
    (line, col)
}

impl Host for Document {
    fn text(&self) -> Rc<str> {
        Rc::clone(&self.text)
    }

    fn root(&self) -> Res<Rc<Tree>> {
        self.tree()
    }

    fn splice(&mut self, start: usize, end: usize, insert: &str) -> Res<()> {
        let before = Rc::clone(&self.text);
        if start > end || !before.is_char_boundary(start) || !before.is_char_boundary(end) {
            return fail(format!("k7x: no splice from {start} to {end}"));
        }
        if insert == &before[start..end] {
            return Ok(());
        }
        let start16 = u16len(&before[..start]);
        let end16 = start16 + u16len(&before[start..end]);
        self.text = format!("{}{insert}{}", &before[..start], &before[end..]).into();
        self.reparse();
        if self.contents.is_some() {
            self.changes.push(Change {
                start,
                end,
                insert: insert.to_owned(),
                start16,
                end16,
            });
            return Ok(());
        }
        let err = self.errors[0].clone();
        self.text = before;
        self.reparse();
        fail(format!("k7x: an edit produced unreadable text ({err})"))
    }
}

impl PathEditor for Document {
    fn set_in(&mut self, path: &[PathSeg], value: &Value) -> Res<()> {
        Document::set_in(self, path, value, WriteOptions::default())
    }

    fn delete_in(&mut self, path: &[PathSeg]) -> Res<bool> {
        Document::delete_in(self, path, false)
    }

    fn insert_in(&mut self, path: &[PathSeg], index: usize, value: &Value) -> Res<()> {
        Document::insert_in(self, path, index, value, WriteOptions::default())
    }

    fn get_source(&self, path: &[PathSeg]) -> Option<String> {
        Document::get_source(self, path).ok().flatten()
    }
}

/// The plain value of a whole file.
///
/// # Errors
/// A `SyntaxError` where the text is not k7x.
pub fn parse(text: &str) -> Result<Value, SyntaxError> {
    let bump = Bump::new();
    Ok(MapTree::from_node(&parse_tree(text, &bump)?).to_value())
}
