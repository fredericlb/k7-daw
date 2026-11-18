// dsp_literal/mod.rs — a DSP module's graph written as k7x data (`dsp.k7x`), decoded into the JSON @studio/dsp's builder prints.
// A reader would get wrong that this evaluates anything: a name reads an EARLIER node, `repeat` copies an item, nothing computes.
// The compile reads it on every build, with no cache; the vocabulary and the parity rules are docs/dsp-literal-plan.md.

mod json;
mod nodes;

use crate::files::Finding;
use indexmap::IndexMap;
use json::J;
use k7x::{Bump, Entry, MapNode, Node, Scalar, parse_tree};
use std::collections::{HashMap, HashSet};

/// The most copies one `repeat` makes: the real loops make 3 to 5.
pub const MAX_REPEAT: usize = 64;

/// The most nodes a graph holds; the largest module has 19.
pub const MAX_NODES: usize = 1024;

/// How deep calls may nest inside one item before an inner one must be named.
pub const MAX_NESTING: usize = 64;

const TOP: [&str; 3] = ["params", "wires", "graph"];
const SPEC: [&str; 4] = ["min", "max", "default", "labels"];
/// The keys a node's map may hold beside its one node word.
const MODIFIERS: [&str; 3] = ["as", "into", "repeat"];

/// The graph JSON of a `dsp.k7x`, byte for byte what the builder's `jsdsp.json` holds; the error names its line and key.
pub fn decode(file: &str, text: &str) -> Result<String, Finding> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    let graph = Ctx::default()
        .root(&root)
        .map_err(|f| Finding::at(file, text, f.at, f.message))?;
    Ok(json::pretty(&graph))
}

/// The reason a `dsp.k7x` does not decode, in the one sentence every host and the TypeScript say: `line N: …`.
pub fn decode_reason(file: &str, text: &str) -> Result<String, String> {
    decode(file, text).map_err(|f| format!("line {}: {}", f.line, f.message))
}

pub(crate) struct Fail {
    at: usize,
    message: String,
}

pub(crate) type R<T> = Result<T, Fail>;

pub(crate) fn fail<T>(at: usize, path: &str, message: impl AsRef<str>) -> R<T> {
    Err(Fail {
        at,
        message: format!("{path}: {}", message.as_ref()),
    })
}

/// Whether a value is read once per block (a param, a literal, a control node) or per sample (a wire).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rate {
    Scalar,
    Audio,
}

/// An input once resolved: a literal, or the name the JSON spells and its rate.
pub(crate) enum Val {
    Num(f64),
    Ref(String, Rate),
}

/// What a node leaves to read: a value, or nothing for a node that acts on the buffer or the notes.
pub(crate) enum Made {
    Value(String, Rate),
    Nothing,
}

#[derive(Default)]
pub(crate) struct Ctx {
    params: HashSet<String>,
    /// Each declared wire, and whether a node has written it yet.
    wires: IndexMap<String, (usize, bool)>,
    bound: HashMap<String, (String, Rate)>,
    /// Every name an `as` gives, anywhere in the graph, so a read of a later one says so.
    named: HashSet<String>,
    counter: usize,
    pub(crate) nodes: Vec<J>,
    depth: usize,
    /// The name the repeated item rebinds, while a `repeat` runs.
    repeating: Option<String>,
}

impl Ctx {
    fn root(&mut self, root: &MapNode) -> R<J> {
        for e in root.entries {
            if !TOP.contains(&e.key) {
                return fail(
                    e.key_start,
                    e.key,
                    "unknown key: a dsp.k7x holds params, wires and graph",
                );
            }
        }
        let params = match entry(root, "params") {
            Some(e) => self.params(e)?,
            None => Vec::new(),
        };
        if let Some(e) = entry(root, "wires") {
            self.wires(e)?;
        }
        if let Some(e) = entry(root, "graph") {
            self.graph(e)?;
        }
        if let Some((name, (at, _))) = self.wires.iter().find(|(_, (_, written))| !written) {
            return fail(
                *at,
                "wires",
                format!("`{name}` is never written: write it with .into(\"{name}\")"),
            );
        }
        Ok(J::Obj(vec![
            ("params", J::Arr(params)),
            ("nodes", J::Arr(std::mem::take(&mut self.nodes))),
        ]))
    }

    fn params(&mut self, e: &Entry) -> R<Vec<J>> {
        let Node::Map(m) = e.value else {
            return fail(e.value.start(), "params", "expected an object of param specs");
        };
        let mut out = Vec::new();
        for p in m.entries {
            let path = format!("params.{}", p.key);
            free_name(p.key, p.key_start, &path)?;
            let Node::Map(spec) = p.value else {
                return fail(p.value.start(), &path, "expected { min, max, default }");
            };
            let mut fields = vec![("name", J::Str(p.key.to_owned()))];
            for key in ["min", "max", "default"] {
                let Some(v) = entry(&spec, key) else {
                    return fail(spec.start, &path, format!("missing `{key}`"));
                };
                fields.push((key, J::Num(finite(&v.value, &format!("{path}.{key}"))?)));
            }
            if let Some(k) = spec.entries.iter().find(|k| !SPEC.contains(&k.key)) {
                return fail(
                    k.key_start,
                    &path,
                    format!("unknown key `{}`: a spec holds min, max, default, labels", k.key),
                );
            }
            if let Some(labels) = entry(&spec, "labels") {
                fields.push(("labels", J::Arr(strings(&labels.value, &format!("{path}.labels"))?)));
            }
            self.params.insert(p.key.to_owned());
            out.push(J::Obj(fields));
        }
        Ok(out)
    }

    fn wires(&mut self, e: &Entry) -> R<()> {
        let names = match e.value {
            Node::List { items, .. } => items,
            ref one => std::slice::from_ref(one),
        };
        for (i, n) in names.iter().enumerate() {
            let path = format!("wires[{i}]");
            let Node::Scalar {
                value: Scalar::Str(name),
                start,
                ..
            } = *n
            else {
                return fail(n.start(), &path, "expected a wire's name");
            };
            free_name(name, start, &path)?;
            if self.params.contains(name) || self.wires.contains_key(name) {
                return fail(start, &path, format!("`{name}` is already a param or a wire"));
            }
            self.wires.insert(name.to_owned(), (start, false));
        }
        Ok(())
    }

    fn graph(&mut self, e: &Entry) -> R<()> {
        let Node::List { items, .. } = e.value else {
            return fail(e.value.start(), "graph", "expected a list of nodes");
        };
        for item in items {
            collect_names(item, &mut self.named);
        }
        for (i, item) in items.iter().enumerate() {
            let path = format!("graph[{i}]");
            let Node::Map(m) = item else {
                return fail(item.start(), &path, "expected a node, such as output(\"x\")");
            };
            match entry(m, "repeat") {
                Some(r) => self.repeat(m, r, &path)?,
                None => {
                    self.call(m, &path)?;
                }
            }
        }
        Ok(())
    }

    fn repeat(&mut self, m: &MapNode, r: &Entry, path: &str) -> R<()> {
        let at = r.value.start();
        let n = finite(&r.value, &format!("{path}.repeat"))?;
        if n.fract() != 0.0 || n < 1.0 || n > MAX_REPEAT as f64 {
            return fail(at, path, format!("repeat takes a whole count from 1 to {MAX_REPEAT}"));
        }
        if entry(m, "into").is_some() {
            return fail(at, path, "a repeated node cannot write a wire: a wire is written once");
        }
        self.repeating = Some(text_of(m, "as", path)?.unwrap_or_default().to_owned());
        for _ in 0..n as usize {
            self.call(m, path)?;
        }
        self.repeating = None;
        Ok(())
    }

    /// One node: its word, its inputs first (each nested call emitted before it), then itself.
    fn call(&mut self, m: &MapNode, path: &str) -> R<Made> {
        let mut word = None;
        for e in m.entries {
            if MODIFIERS.contains(&e.key) {
                continue;
            }
            if !nodes::WORDS.contains(&e.key) {
                return fail(
                    e.key_start,
                    path,
                    format!("unknown node type `{}`; {}", e.key, nodes::vocabulary()),
                );
            }
            if let Some(first) = word.replace(e) {
                return fail(
                    e.key_start,
                    path,
                    format!(
                        "`{}` and `{}` are two nodes: nest one inside the other",
                        first.key, e.key
                    ),
                );
            }
        }
        let Some(word) = word else {
            return fail(m.start, path, "a node names its type, such as amul(\"x\", 0.5)");
        };
        let path = format!("{path}.{}", word.key);
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return fail(
                word.key_start,
                &path,
                format!("calls nest deeper than {MAX_NESTING}: name an inner node with .as(…)"),
            );
        }
        let into = self.wire_target(m, word, &path)?;
        let name = self.name(m, &path)?;
        let made = nodes::emit(self, word, &path, into.as_deref())?;
        self.depth -= 1;
        if self.nodes.len() > MAX_NODES {
            return fail(
                word.key_start,
                &path,
                format!("the graph holds more than {MAX_NODES} nodes"),
            );
        }
        if let Some(slot) = into.and_then(|w| self.wires.get_mut(&w)) {
            slot.1 = true;
        }
        if let Some((name, at)) = name {
            let Made::Value(id, rate) = &made else {
                return fail(at, &path, format!("`{}` leaves no value to name", word.key));
            };
            self.bound.insert(name, (id.clone(), *rate));
        }
        Ok(made)
    }

    fn wire_target(&self, m: &MapNode, word: &Entry, path: &str) -> R<Option<String>> {
        let Some(w) = text_of(m, "into", path)? else {
            return Ok(None);
        };
        let at = entry(m, "into").map_or(m.start, |e| e.value.start());
        if !nodes::WRITES_WIRE.contains(&word.key) {
            return fail(at, path, "only allpass and delay write a wire");
        }
        if entry(m, "as").is_some() {
            return fail(
                at,
                path,
                "a node that writes a wire is read by the wire's name, not an `as`",
            );
        }
        match self.wires.get(w) {
            None => fail(at, path, format!("`{w}` is not a wire: declare it in wires(…)")),
            Some((_, true)) => fail(
                at,
                path,
                format!("wire `{w}` is already written: a wire has one writer"),
            ),
            Some(_) => Ok(Some(w.to_owned())),
        }
    }

    fn name(&self, m: &MapNode, path: &str) -> R<Option<(String, usize)>> {
        let Some(name) = text_of(m, "as", path)? else {
            return Ok(None);
        };
        let at = entry(m, "as").map_or(m.start, |e| e.value.start());
        free_name(name, at, path)?;
        let rebinding = self.repeating.as_deref() == Some(name) && self.depth == 1;
        if self.repeating.is_some() && !rebinding {
            return fail(at, path, "inside a repeat, only the repeated node itself is named");
        }
        if self.params.contains(name) || self.wires.contains_key(name) {
            return fail(at, path, format!("`{name}` is already a param or a wire"));
        }
        if self.bound.contains_key(name) && !rebinding {
            return fail(at, path, format!("`{name}` names an earlier node already"));
        }
        Ok(Some((name.to_owned(), at)))
    }

    /// An input: a literal, a name, or a nested call emitted now.
    pub(crate) fn input(&mut self, n: &Node, path: &str) -> R<Val> {
        match *n {
            Node::Scalar {
                value: Scalar::Num(_), ..
            } => Ok(Val::Num(finite(n, path)?)),
            Node::Scalar {
                value: Scalar::Str(s),
                start,
                ..
            } => self.resolve(s, start, path),
            Node::Map(m) => match self.call(&m, path)? {
                Made::Value(id, rate) => Ok(Val::Ref(id, rate)),
                Made::Nothing => fail(m.start, path, "this node leaves no value to read"),
            },
            Node::List { start, .. } => fail(start, path, "a list is not an input: write a call, such as mul(a, b)"),
            Node::Scalar { start, .. } => fail(start, path, "expected a number, a name or a node"),
        }
    }

    fn resolve(&self, name: &str, at: usize, path: &str) -> R<Val> {
        if let Some((id, rate)) = self.bound.get(name) {
            return Ok(Val::Ref(id.clone(), *rate));
        }
        if self.wires.contains_key(name) {
            return Ok(Val::Ref(name.to_owned(), Rate::Audio));
        }
        if self.params.contains(name) || name == "sr" {
            return Ok(Val::Ref(name.to_owned(), Rate::Scalar));
        }
        if self.named.contains(name) {
            return fail(
                at,
                path,
                format!(
                    "`{name}` is named further down: a node reads only what comes before it, unless it is a wire (wires(…) and .into(…))"
                ),
            );
        }
        fail(
            at,
            path,
            format!("`{name}` names nothing: no param, node or wire is called that"),
        )
    }

    /// The next node id, `n0`, `n1`…, as the builder counts them.
    pub(crate) fn fresh(&mut self) -> String {
        let id = format!("n{}", self.counter);
        self.counter += 1;
        id
    }
}

fn entry<'a>(m: &MapNode<'a>, key: &str) -> Option<&'a Entry<'a>> {
    m.entries.iter().find(|e| e.key == key)
}

fn text_of<'a>(m: &MapNode<'a>, key: &str, path: &str) -> R<Option<&'a str>> {
    match entry(m, key).map(|e| e.value) {
        None => Ok(None),
        Some(Node::Scalar {
            value: Scalar::Str(s), ..
        }) => Ok(Some(s)),
        Some(v) => fail(v.start(), path, format!("`{key}` takes a name")),
    }
}

pub(crate) fn finite(n: &Node, path: &str) -> R<f64> {
    match *n {
        Node::Scalar {
            value: Scalar::Num(v), ..
        } if v.is_finite() => Ok(v),
        _ => fail(n.start(), path, "expected a finite number"),
    }
}

fn strings(n: &Node, path: &str) -> R<Vec<J>> {
    let Node::List { items, .. } = *n else {
        return fail(n.start(), path, "expected a list of labels");
    };
    items
        .iter()
        .map(|i| match *i {
            Node::Scalar {
                value: Scalar::Str(s), ..
            } => Ok(J::Str(s.to_owned())),
            _ => fail(i.start(), path, "expected a label"),
        })
        .collect()
}

/// A name the JSON can spell without meeting another: not empty, not `sr`, not a node id such as `n3`.
fn free_name(name: &str, at: usize, path: &str) -> R<()> {
    let id_like = name.len() > 1 && name.starts_with('n') && name[1..].bytes().all(|b| b.is_ascii_digit());
    if name.is_empty() || name == "sr" || id_like {
        return fail(
            at,
            path,
            format!("`{name}` cannot be a name: empty, `sr` and node ids such as n3 are taken"),
        );
    }
    Ok(())
}

fn collect_names(n: &Node, into: &mut HashSet<String>) {
    match *n {
        Node::Map(m) => {
            for e in m.entries {
                if let (
                    "as",
                    Node::Scalar {
                        value: Scalar::Str(s), ..
                    },
                ) = (e.key, e.value)
                {
                    into.insert(s.to_owned());
                }
                collect_names(&e.value, into);
            }
        }
        Node::List { items, .. } => items.iter().for_each(|i| collect_names(i, into)),
        Node::Scalar { .. } => {}
    }
}
