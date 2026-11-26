// emit_ctx.rs — what every part of the emitter shares: the k7x files it writes, the slugs it names files by, and the one bag of name spaces.
// What a reader gets wrong: the name spaces are SHARED on purpose, a lane, a module and a node can never take one name twice.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use k7x::PathSeg;
use k7x::edit::{StringifyOptions, Value, js_trim, stringify};

use crate::model::{Bytes, Song};

/// How a file is laid out: a comment on top, and the nodes kept on one line.
#[derive(Default)]
pub struct DocLayout<'a> {
    pub note: Option<&'a str>,
    /// Nodes written on one line, by their path from the value.
    pub one_line: Vec<Vec<String>>,
    /// Lists whose every item is written on one line.
    pub items_one_line: Vec<Vec<String>>,
}

/// A map value, ordered as a JS object holds its keys.
#[must_use]
pub fn obj(entries: Vec<(&str, Value)>) -> Value {
    Value::object(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

#[must_use]
pub fn text(s: &str) -> Value {
    Value::Str(s.to_owned())
}

#[must_use]
pub fn number(n: f64) -> Value {
    Value::Num(n)
}

/// The note's lines, each stripped of the control characters a name from the imported file may carry.
fn note_lines(note: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut chars = note.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut cur));
            }
            '\n' | '\u{85}' | '\u{2028}' | '\u{2029}' => lines.push(std::mem::take(&mut cur)),
            c if matches!(c, '\u{0}'..='\u{8}' | '\u{b}'..='\u{1f}' | '\u{7f}') => {}
            c => cur.push(c),
        }
    }
    lines.push(cur);
    lines
}

fn is_key(seg: &PathSeg, want: &str) -> bool {
    matches!(seg, PathSeg::Key(k) if k == want)
}

fn same_path(path: &[PathSeg], want: &[String]) -> bool {
    path.len() == want.len() && path.iter().zip(want).all(|(s, w)| is_key(s, w))
}

/// A whole new k7x file's text, in the canonical layout.
pub fn print_doc(value: &Value, layout: &DocLayout<'_>) -> String {
    let header: Vec<String> = layout.note.map(note_lines).unwrap_or_default();
    let inline = |p: &[PathSeg], _v: &Value| {
        layout.one_line.iter().any(|o| same_path(p, o))
            || (matches!(p.last(), Some(PathSeg::Index(_)))
                && layout.items_one_line.iter().any(|o| same_path(&p[..p.len() - 1], o)))
    };
    stringify(
        value,
        &StringifyOptions {
            header: &header,
            inline: Some(&inline),
        },
    )
    .unwrap_or_default()
}

/// A snake_case, filesystem-safe base: every file created on import is named this way.
#[must_use]
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for c in js_trim(name).to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending && !out.is_empty() {
                out.push('_');
            }
            pending = false;
            out.push(c);
        } else {
            pending = true;
        }
    }
    if out.is_empty() { "node".into() } else { out }
}

/// `base`, or `base_2`, `base_3`… until `used` (lowercase) does not hold it; the winner is then taken.
pub fn uniquify(base: &str, used: &mut HashSet<String>) -> String {
    let mut s = base.to_owned();
    let mut n = 2;
    while used.contains(&s.to_lowercase()) {
        s = format!("{base}_{n}");
        n += 1;
    }
    used.insert(s.to_lowercase());
    s
}

fn meter(song: &Song) -> (f64, f64) {
    let mut parts = song.time_signature.split('/');
    let mut read = |d: f64| {
        parts
            .next()
            .and_then(|p| crate::xml::parse_float(p))
            .filter(|n| *n != 0.0)
            .map_or(d, f64::trunc)
    };
    let n = read(4.0);
    (n, read(4.0))
}

pub struct Step {
    pub module: String,
    /// `None` is on.
    pub enabled: Option<bool>,
}

pub struct OutDef {
    pub to: String,
    /// `None` or 1 is the whole signal; below it a weighted send.
    pub gain: Option<f64>,
}

pub struct ChainDef {
    pub from: Option<String>,
    pub uses: Vec<Step>,
    pub enabled: Option<bool>,
    pub out: Vec<OutDef>,
}

/// One use of a plugin: its settings and the node file they land in; `node` is empty once collapsed.
pub struct InsertDef {
    pub node: String,
    pub params: Vec<(String, f64)>,
    pub preset: Option<String>,
}

/// A self-contained module: one per PLUGIN, one insert per use.
pub struct ModuleDef {
    pub name: String,
    pub folder: &'static str,
    pub plugin_type: String,
    pub plugin_name: String,
    pub inserts: Vec<InsertDef>,
}

/// What an import writes: texts and binaries by path from the workspace's root, in the order they were made.
#[derive(Default)]
pub struct Written {
    pub texts: Vec<(String, String)>,
    pub binaries: Vec<(String, Bytes)>,
}

/// Everything one import shares between its parts: the name spaces, what it built, what it counted.
pub struct EmitCtx<'a> {
    /// The new project's folder, below the workspace.
    pub project: String,
    pub song: &'a Song,
    pub assets: HashMap<&'a str, &'a Bytes>,
    pub slug_of: HashMap<String, String>,
    pub used_node_names: HashSet<String>,
    pub used_chain_names: HashSet<String>,
    pub used_clip_ids: HashSet<String>,
    pub used_module_names: HashSet<String>,
    pub module_of: HashMap<String, usize>,
    pub modules: Vec<ModuleDef>,
    pub chains: IndexMap<String, ChainDef>,
    pub bus_chains: HashSet<String>,
    /// Content hash to where those bytes landed, and the file names already taken.
    pub audio_rel: HashMap<String, String>,
    pub taken_audio: HashSet<String>,
    pub missing_audio: Vec<String>,
    pub stand_ins: usize,
    pub clip_files: usize,
    pub sequence_files: usize,
    pub out: Written,
    pub meter: (f64, f64),
}

impl<'a> EmitCtx<'a> {
    /// A fresh context, its name spaces seeded with the lanes' slugs and the workspace's modules.
    pub fn new(
        project: &str,
        song: &'a Song,
        assets: &'a [(String, Bytes)],
        taken_modules: &HashSet<String>,
    ) -> EmitCtx<'a> {
        let mut used = HashSet::new();
        let mut slug_of = HashMap::new();
        for t in &song.tracks {
            slug_of.insert(t.id.clone(), uniquify(&slugify(&t.name), &mut used));
        }
        EmitCtx {
            project: project.to_owned(),
            song,
            assets: assets.iter().map(|(k, v)| (k.as_str(), v)).collect(),
            slug_of,
            used_node_names: used,
            used_chain_names: HashSet::from(["out".to_owned()]),
            used_clip_ids: HashSet::new(),
            used_module_names: taken_modules.clone(),
            module_of: HashMap::new(),
            modules: Vec::new(),
            chains: IndexMap::new(),
            bus_chains: HashSet::new(),
            audio_rel: HashMap::new(),
            taken_audio: HashSet::new(),
            missing_audio: Vec::new(),
            stand_ins: 0,
            clip_files: 0,
            sequence_files: 0,
            out: Written::default(),
            meter: meter(song),
        }
    }

    /// A text file of the project, `rel` below its folder.
    pub fn write_doc(&mut self, rel: &str, value: &Value, layout: &DocLayout<'_>) {
        let path = format!("{}/{rel}", self.project);
        self.out.texts.push((path, print_doc(value, layout)));
    }

    /// A text file of the workspace's shared `_modules`, `rel` below it.
    pub fn write_shared_doc(&mut self, rel: &str, value: &Value, layout: &DocLayout<'_>) {
        self.out
            .texts
            .push((format!("_modules/{rel}"), print_doc(value, layout)));
    }

    pub fn write_binary(&mut self, path: String, bytes: Bytes) {
        self.out.binaries.push((path, bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_snake_case_slug_and_collisions_get_a_suffix() {
        assert_eq!(slugify("  Pro-Q 3 "), "pro_q_3");
        assert_eq!(slugify("!!!"), "node");
        let mut used = HashSet::new();
        assert_eq!(uniquify("a", &mut used), "a");
        assert_eq!(uniquify("A", &mut used), "A_2");
    }

    #[test]
    fn a_note_loses_its_control_characters_and_splits_on_every_line_end() {
        assert_eq!(note_lines("a\u{7}b\r\nc\u{2028}d"), ["ab", "c", "d"]);
    }
}
