// scope.rs — a view of a document under one base path, so a writer that owns a subtree addresses it from its own root.
// A reader would get wrong that it holds a node: it holds a path, resolved again on every call, so it survives every edit.

use super::apply::PathEditor;
use super::document::{Document, WriteOptions};
use super::layout::Res;
use super::tree::Tree;
use super::value::Value;
use crate::ast::PathSeg;

pub struct Scope<'a> {
    doc: &'a mut Document,
    base: Vec<PathSeg>,
}

impl<'a> Scope<'a> {
    pub(super) fn new(doc: &'a mut Document, base: Vec<PathSeg>) -> Scope<'a> {
        Scope { doc, base }
    }

    /// The path this view is rooted at; empty for a whole document.
    #[must_use]
    pub fn base(&self) -> &[PathSeg] {
        &self.base
    }

    /// The document under the view, for a writer that applies a whole value through the file's short forms.
    pub fn document(&mut self) -> &mut Document {
        self.doc
    }

    fn path(&self, path: &[PathSeg]) -> Vec<PathSeg> {
        [self.base.as_slice(), path].concat()
    }

    pub fn get_in(&self, path: &[PathSeg]) -> Res<Option<Value>> {
        self.doc.get_in(&self.path(path))
    }

    pub fn has_in(&self, path: &[PathSeg]) -> Res<bool> {
        self.doc.has_in(&self.path(path))
    }

    pub fn node_at<R>(&self, path: &[PathSeg], f: impl FnOnce(Option<&Tree>) -> R) -> Res<R> {
        self.doc.node_at(&self.path(path), f)
    }

    pub fn get_source(&self, path: &[PathSeg]) -> Res<Option<String>> {
        self.doc.get_source(&self.path(path))
    }

    /// The value at the base.
    pub fn to_js(&self) -> Res<Option<Value>> {
        self.doc.get_in(&self.base)
    }

    pub fn set_in(&mut self, path: &[PathSeg], value: &Value, opts: WriteOptions) -> Res<()> {
        self.doc.set_in(&self.path(path), value, opts)
    }

    pub fn add_in(&mut self, path: &[PathSeg], value: &Value, opts: WriteOptions) -> Res<()> {
        self.doc.add_in(&self.path(path), value, opts)
    }

    pub fn insert_in(
        &mut self,
        path: &[PathSeg],
        index: usize,
        value: &Value,
        opts: WriteOptions,
    ) -> Res<()> {
        self.doc.insert_in(&self.path(path), index, value, opts)
    }

    pub fn delete_in(&mut self, path: &[PathSeg], prune: bool) -> Res<bool> {
        self.doc.delete_in(&self.path(path), prune)
    }

    pub fn apply_in(&mut self, path: &[PathSeg], value: &Value) -> Res<()> {
        self.doc.apply_in(&self.path(path), value)
    }

    pub fn rename_key(&mut self, path: &[PathSeg], new_key: &str) -> Res<()> {
        self.doc.rename_key(&self.path(path), new_key)
    }

    pub fn set_source_in(&mut self, path: &[PathSeg], source: &str) -> Res<()> {
        self.doc.set_source_in(&self.path(path), source)
    }

    pub fn insert_source_in(&mut self, path: &[PathSeg], index: usize, source: &str) -> Res<()> {
        self.doc.insert_source_in(&self.path(path), index, source)
    }

    pub fn move_in(&mut self, path: &[PathSeg], from: usize, to: usize) -> Res<()> {
        self.doc.move_in(&self.path(path), from, to)
    }

    pub fn position_of(&self, path: &[PathSeg]) -> Res<Option<(usize, usize)>> {
        self.doc.position_of(&self.path(path))
    }

    /// A narrower view, its base relative to this one.
    pub fn scope(&mut self, path: &[PathSeg]) -> Scope<'_> {
        let base = self.path(path);
        Scope {
            doc: self.doc,
            base,
        }
    }
}

impl PathEditor for Scope<'_> {
    fn set_in(&mut self, path: &[PathSeg], value: &Value) -> Res<()> {
        Scope::set_in(self, path, value, WriteOptions::default())
    }

    fn delete_in(&mut self, path: &[PathSeg]) -> Res<bool> {
        Scope::delete_in(self, path, false)
    }

    fn insert_in(&mut self, path: &[PathSeg], index: usize, value: &Value) -> Res<()> {
        Scope::insert_in(self, path, index, value, WriteOptions::default())
    }

    fn get_source(&self, path: &[PathSeg]) -> Option<String> {
        Scope::get_source(self, path).ok().flatten()
    }
}
