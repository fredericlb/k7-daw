// files/validate.rs — can the compiler read this text? One router over the file readers, so an editor asks a buffer what a loader asks a file.
// A reader would get wrong that the schema IS the reader: the compile's own decoder judges, so a file is refused for the reason it would be at compile.

use super::desk::{decode_mix, decode_mix_view};
use super::preset_file::{decode_config, decode_preset};
use super::route::{Family, family_of, is_k7x};
use super::{Finding, decode, decode_clip, decode_root, line_col};
use crate::curves::decode_curve;
use crate::modules::decode_module;
use crate::modules::panels::{decode_controller, decode_ui_file};
use crate::routing::{decode_chain, decode_graph};
use crate::{PathRule, Sequence, Settings, Timeline};
use k7x::{Bump, MapNode, Node, parse_tree};
use serde::Serialize;

/// How far the check could go: what a caller may conclude from `ok`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ValidityDepth {
    /// Parsed and checked against this file's schema.
    Schema,
    /// Parsed; no schema knows this path, so only malformed text is caught.
    Syntax,
    /// Not a workspace document at all: nothing here can judge it.
    None,
}

/// A problem in a file: its dotted key path, its sentence, and its place when the file says so (1-based, columns in UTF-16 units).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModuleIssue {
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub col: Option<u32>,
}

/// What a caller may conclude about a text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Validity {
    /// Nothing found. A file this does not recognise is `ok`: silence is not a verdict.
    pub ok: bool,
    pub issues: Vec<ModuleIssue>,
    pub depth: ValidityDepth,
    /// The text does not PARSE, as opposed to parsing and failing its schema.
    pub malformed: bool,
}

fn verdict(issues: Vec<ModuleIssue>, depth: ValidityDepth, malformed: bool) -> Validity {
    Validity {
        ok: issues.is_empty(),
        issues,
        depth,
        malformed,
    }
}

/// The byte offset of a 1-based line and a 1-based UTF-16 column.
fn offset_of(text: &str, line: u32, col: u32) -> usize {
    let mut start = 0;
    for _ in 1..line {
        match text[start..].find('\n') {
            Some(i) => start += i + 1,
            None => return text.len(),
        }
    }
    let mut units = 1;
    for (i, c) in text[start..].char_indices() {
        if units >= col || c == '\n' {
            return start + i;
        }
        units += c.len_utf16() as u32;
    }
    text.len()
}

/// The steps down to the deepest node that holds `offset`, each with where it is written: a key at its key, an item at its start.
fn walk(node: &Node, offset: usize, out: &mut Vec<(String, usize)>) {
    match node {
        Node::Map(m) => walk_map(m, offset, out),
        Node::List { items, .. } => {
            let held = |(_, n): &(usize, &Node)| (n.start()..n.end().max(n.start() + 1)).contains(&offset);
            if let Some((i, item)) = items.iter().enumerate().find(held) {
                out.push((i.to_string(), item.start()));
                walk(item, offset, out);
            }
        }
        Node::Scalar { .. } => {}
    }
}

fn walk_map(m: &MapNode, offset: usize, out: &mut Vec<(String, usize)>) {
    let held = |e: &&k7x::Entry| (e.key_start..e.end.max(e.value.end()).max(e.key_end)).contains(&offset);
    if let Some(e) = m.entries.iter().rev().find(held) {
        out.push((e.key.to_owned(), e.key_start));
        if offset >= e.key_end {
            walk(&e.value, offset, out);
        }
    }
}

/// The key a "missing key `x`" sentence names.
fn missing_key(message: &str) -> Option<&str> {
    message.strip_prefix("missing key `")?.split('`').next()
}

/// A finding as an issue: the dotted path it is about and, where a path exists, the place of its last step.
fn issue(text: &str, f: &Finding, path: Option<&str>) -> ModuleIssue {
    let mut steps = Vec::new();
    let tail = missing_key(&f.message).map(str::to_owned);
    if path.is_none() {
        let bump = Bump::new();
        if let Ok(root) = parse_tree(text, &bump) {
            let offset = offset_of(text, f.line, f.col);
            // A key missing from the root is placed at the root, which an entry written first also starts at.
            if tail.is_none() || offset != root.start {
                walk_map(&root, offset, &mut steps);
            }
        }
    }
    let (line, col) = steps.last().map_or((f.line, f.col), |(_, at)| line_col(text, *at));
    let mut names: Vec<&str> = steps.iter().map(|(s, _)| s.as_str()).collect();
    names.extend(tail.as_deref());
    ModuleIssue {
        path: path.map_or_else(|| names.join("."), str::to_owned),
        message: f.message.clone(),
        line: Some(line),
        col: Some(col),
    }
}

/// The reader of one family over a text; a project root answers its refusal as a finding too.
fn judge(family: Family, path: &str, text: &str) -> Result<(), Finding> {
    match family {
        Family::Preset => decode_preset(path, text).map(drop),
        Family::Ui => decode_ui_file(path, text).map(drop),
        Family::Controller => decode_controller(path, text).map(drop),
        Family::Config => decode_config(path, text).map(drop),
        Family::Module => decode_module(path, text).map(drop),
        Family::Chain => decode_chain(path, text, PathRule::HOST).map(drop),
        Family::Graph => decode_graph(path, text).map(drop),
        Family::Clip => decode_clip(path, text).map(drop),
        Family::Sequence => decode::<Sequence>(path, text).map(drop),
        Family::Curve => decode_curve(path, text).map(drop),
        Family::Timeline => decode::<Timeline>(path, text).map(drop),
        Family::MixView => decode_mix_view(path, text).map(drop),
        Family::Mix => decode_mix(path, text).map(drop),
        Family::Settings => decode::<Settings>(path, text).map(drop),
        Family::Project => decode_root(path, text).and_then(|r| r.too_new.map_or(Ok(()), |t| Err(t.finding))),
    }
}

/// Is `text` a document the compiler could read at `path`? A newer format first, then syntax, then the schema; ONE finding, the first.
pub fn validate_file(path: &str, text: &str) -> Validity {
    if !is_k7x(path) {
        return verdict(Vec::new(), ValidityDepth::None, false);
    }
    let family = family_of(path);
    let bump = Bump::new();
    let syntax = parse_tree(text, &bump).err();
    // A newer project is refused from its text before any parse, so it says "update k7" even when the rest would not read.
    if family == Some(Family::Project)
        && let Ok(root) = decode_root(path, text)
        && let Some(t) = root.too_new
    {
        return verdict(
            vec![issue(text, &t.finding, Some("format"))],
            ValidityDepth::Schema,
            false,
        );
    }
    if let Some(e) = syntax {
        return verdict(
            vec![issue(text, &Finding::syntax(path, &e), Some(""))],
            ValidityDepth::Syntax,
            true,
        );
    }
    let Some(family) = family else {
        return verdict(Vec::new(), ValidityDepth::Syntax, false);
    };
    match judge(family, path, text) {
        Ok(()) => verdict(Vec::new(), ValidityDepth::Schema, false),
        Err(f) => verdict(vec![issue(text, &f, None)], ValidityDepth::Schema, false),
    }
}
