// sugar.rs — the short forms a .k7x file may write (a sequence's `notes("c4 d4")`, a graph's `runs`), and the path rule that picks them.
// A reader would get wrong `isDocFile`: its regex leaves the dot of `.k7x` unescaped, so `x_k7x` is a document name there and here.

use crate::error::{Error, Res, fail};
use crate::notes::expand_notes;
use k7x::edit::{Sugar, SugarPart, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SugarKind {
    Sequence,
    Segments,
}

/// One kind's short forms: `Sugar` for the editor, and `try_expand` for a caller that wants the error.
pub struct FileSugar {
    kind: SugarKind,
    parts: Vec<SugarPart>,
}

/// What `fileKindFor` answers for a project-relative path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Module,
    Chains,
    Chain,
    Clip,
    Sequence,
    Curve,
    Project,
}

fn has_segment(p: &str, seg: &str) -> bool {
    p.starts_with(seg) || p.contains(&format!("/{seg}"))
}

/// `/(^|\/)<name>.k7x$/i` with the dot as any character but a line end, as `docPattern` builds it.
fn loose_name(p: &str, name: &str) -> bool {
    let lower = p.to_ascii_lowercase();
    let Some(head) = lower.strip_suffix("k7x") else {
        return false;
    };
    let Some(c) = head.chars().next_back() else {
        return false;
    };
    if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
        return false;
    }
    let s = &head[..head.len() - c.len_utf8()];
    s.strip_suffix(name)
        .is_some_and(|before| before.is_empty() || before.ends_with('/'))
}

/// `isDocFile`: a name ending in `.k7x` any case, the dot being any character.
#[must_use]
pub fn is_doc_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.strip_suffix("k7x").is_some_and(|h| {
        h.chars()
            .next_back()
            .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    })
}

/// `stripDocExt`: the name without its last four characters when `is_doc_file`.
#[must_use]
pub fn strip_doc_ext(name: &str) -> &str {
    if !is_doc_file(name) {
        return name;
    }
    let cut = name.char_indices().rev().nth(3).map_or(0, |(i, _)| i);
    &name[..cut]
}

/// Which kind a project-relative path is, by folder and not by extension.
#[must_use]
pub fn file_kind_for(path: &str) -> Option<FileKind> {
    let p = path.replace('\\', "/");
    if p.is_empty() {
        return None;
    }
    if ["modules/", "nodes/", "tracks/"].iter().any(|s| has_segment(&p, s)) {
        return Some(FileKind::Module);
    }
    let lower = p.to_ascii_lowercase();
    let chain_rest: Vec<&str> = lower
        .match_indices("chains/")
        .filter(|(i, _)| *i == 0 || lower.as_bytes()[*i - 1] == b'/')
        .map(|(i, _)| &lower[i + 7..])
        .collect();
    if chain_rest
        .iter()
        .any(|r| *r == "graph.k7x" || (r.ends_with("/graph.k7x") && r.len() > "/graph.k7x".len()))
    {
        return Some(FileKind::Chains);
    }
    if chain_rest.iter().any(|r| r.len() > 4 && r.ends_with(".k7x")) {
        return Some(FileKind::Chain);
    }
    if has_segment(&p, "clips/") {
        return Some(FileKind::Clip);
    }
    if has_segment(&p, "sequences/") {
        return Some(FileKind::Sequence);
    }
    if has_segment(&p, "curves/") {
        return Some(FileKind::Curve);
    }
    let tail = |name: &str| lower == name || lower.ends_with(&format!("/{name}"));
    if tail("chains.k7x") {
        return Some(FileKind::Chains);
    }
    tail("project.k7x").then_some(FileKind::Project)
}

/// Which short form a path's file is written in, or None.
#[must_use]
pub fn sugar_kind(path: &str) -> Option<SugarKind> {
    if file_kind_for(path) == Some(FileKind::Sequence) {
        return Some(SugarKind::Sequence);
    }
    let p = path.replace('\\', "/");
    let one_below_chains = {
        let name_after = |i: usize| !p[i..].is_empty() && !p[i..].contains('/');
        p.match_indices("chains/")
            .any(|(i, _)| (i == 0 || p.as_bytes()[i - 1] == b'/') && name_after(i + 7))
    };
    (loose_name(&p, "graph") || (loose_name(&p, "chains") && !one_below_chains)).then_some(SugarKind::Segments)
}

/// The short forms a file at `path` may write, or None for a kind that has none.
#[must_use]
pub fn sugar_for(path: &str) -> Option<FileSugar> {
    let kind = sugar_kind(path)?;
    let part = match kind {
        SugarKind::Sequence => SugarPart {
            key: "notes".to_owned(),
            target: "notes".to_owned(),
            present: Box::new(|raw| match raw.get("notes") {
                Some(Value::Str(_)) => true,
                Some(n @ Value::Map(_)) => matches!(n.get("seq"), Some(Value::Str(_))),
                _ => false,
            }),
        },
        SugarKind::Segments => SugarPart {
            key: "runs".to_owned(),
            target: "modules".to_owned(),
            present: Box::new(|raw| raw.get("runs").is_some()),
        },
    };
    Some(FileSugar {
        kind,
        parts: vec![part],
    })
}

/// `{...raw, key: value}`: the key keeps its place when `raw` holds it, else goes last.
fn with_key(raw: &Value, key: &str, value: Value) -> Value {
    let Value::Map(m) = raw else { return raw.clone() };
    let mut out = m.clone();
    match out.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = value,
        None => out.push((key.to_owned(), value)),
    }
    Value::Map(out)
}

fn expand_sequence(raw: &Value) -> Res<Value> {
    let notes = match raw.get("notes") {
        Some(Value::Str(s)) => expand_notes(s, 1.0, None)?,
        Some(n @ Value::Map(m)) if matches!(n.get("seq"), Some(Value::Str(_))) => {
            let extra: Vec<&str> = m
                .iter()
                .map(|(k, _)| k.as_str())
                .filter(|k| !["seq", "step", "v"].contains(k))
                .collect();
            if !extra.is_empty() {
                return fail(format!("notes: seq(…) takes step and v, not {}", extra.join(", ")));
            }
            let step = match n.get("step") {
                None => None,
                Some(Value::Num(x)) if *x > 0.0 => Some(*x),
                Some(_) => return fail("notes: step is a number of beats above 0"),
            };
            let v = match n.get("v") {
                None => None,
                Some(Value::Num(x)) if (0.0..=1.0).contains(x) => Some(*x),
                Some(_) => return fail("notes: v is a velocity from 0 to 1"),
            };
            let Some(Value::Str(seq)) = n.get("seq") else {
                return Ok(raw.clone());
            };
            expand_notes(seq, step.unwrap_or(1.0), v)?
        }
        _ => return Ok(raw.clone()),
    };
    Ok(with_key(raw, "notes", Value::List(notes)))
}

fn expand_graph(raw: &Value) -> Res<Value> {
    let Some(runs) = raw.get("runs") else {
        return Ok(raw.clone());
    };
    let bad = "runs: a list of runs, each a list of two or more node names";
    let Value::List(runs) = runs else { return fail(bad) };
    let mut lists: Vec<Vec<&str>> = Vec::new();
    for r in runs {
        match r {
            Value::List(items) if items.len() >= 2 => {
                let names: Option<Vec<&str>> = items
                    .iter()
                    .map(|x| if let Value::Str(s) = x { Some(s.as_str()) } else { None })
                    .collect();
                lists.push(names.ok_or_else(|| Error::Msg(bad.to_owned()))?);
            }
            _ => return fail(bad),
        }
    }
    let Value::Map(entries) = raw else {
        return Ok(raw.clone());
    };
    let rest = Value::Map(entries.iter().filter(|(k, _)| k != "runs").cloned().collect());
    let mut modules: Vec<(String, Value)> = match rest.get("modules") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    };
    for run in lists {
        for pair in run.windows(2) {
            let at = match modules.iter().position(|(k, _)| k == pair[0]) {
                Some(i) => i,
                None => {
                    modules.push((pair[0].to_owned(), Value::Null));
                    modules.len() - 1
                }
            };
            if matches!(modules[at].1, Value::Null) {
                modules[at].1 = Value::List(vec![]);
            }
            match &mut modules[at].1 {
                Value::List(d) => {
                    if !d.contains(&Value::Str(pair[1].to_owned())) {
                        d.push(Value::Str(pair[1].to_owned()));
                    }
                }
                Value::Str(s) if s.contains(pair[1]) => {}
                Value::Str(_) => return fail("dests.push is not a function"),
                _ => return fail("dests.includes is not a function"),
            }
        }
    }
    Ok(with_key(&rest, "modules", Value::object(modules)))
}

impl FileSugar {
    #[must_use]
    pub fn kind(&self) -> SugarKind {
        self.kind
    }

    /// The data with every short form expanded.
    ///
    /// # Errors
    /// The sentence naming what does not read.
    pub fn try_expand(&self, raw: &Value) -> Res<Value> {
        match self.kind {
            SugarKind::Sequence => expand_sequence(raw),
            SugarKind::Segments => expand_graph(raw),
        }
    }
}

impl Sugar for FileSugar {
    fn parts(&self) -> &[SugarPart] {
        &self.parts
    }

    // The editor's trait cannot fail: a caller that wants the error asks `try_expand` first.
    fn expand(&self, raw: &Value) -> Value {
        self.try_expand(raw).unwrap_or_else(|_| raw.clone())
    }
}
