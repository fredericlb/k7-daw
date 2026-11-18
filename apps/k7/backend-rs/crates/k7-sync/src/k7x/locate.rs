//! locate.rs — where a key path leads in a k7x tree, and the keys that name a list's items (`listKeys.ts`, `k7xOps.ts`).
//! A path never holds a list index: an item is `[field=value,...]` over its list's key fields, or the list is a leaf.
//! What a reader gets wrong: offsets are bytes here, where the TypeScript counted UTF-16 units; fragments are the same text.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use k7x::edit::{Value, js_number, json_text};
use k7x::{Node, PathSeg, Scalar};

/// A list's map key and the fields that make one of its items unique (song-schema).
const LIST_KEYS: &[(&str, &[&str])] = &[
    ("events", &["ref", "start"]),
    ("notes", &["t", "k"]),
    ("points", &["time"]),
    ("takes", &["ref", "start"]),
    ("comp", &["from"]),
    ("crossfades", &["at"]),
    ("automations", &["param"]),
    ("subchains", &["id"]),
];

pub fn list_keys(name: &str) -> Option<&'static [&'static str]> {
    LIST_KEYS.iter().find(|(k, _)| *k == name).map(|(_, f)| *f)
}

/// A value's own k7x source; an empty call's zero-width `true` is spelled out.
pub fn fragment<'t>(text: &'t str, node: &Node) -> &'t str {
    if node.start() == node.end() {
        "true"
    } else {
        &text[node.start()..node.end()]
    }
}

fn scalar_text(s: &Scalar) -> String {
    match s {
        Scalar::Str(v) => json_text(&Value::Str((*v).to_owned())),
        Scalar::Num(n) => js_number(*n),
        Scalar::Bool(b) => b.to_string(),
        Scalar::Null => "null".to_owned(),
    }
}

/// Each item's segment in order, or None when the list under `name` is not keyed (listKeys.ts).
pub fn item_segments(name: Option<&str>, list: Option<&Node>) -> Option<Vec<String>> {
    let Some(Node::List { items, .. }) = list else {
        return None;
    };
    let fields = list_keys(name?)?;
    let mut out: Vec<String> = Vec::with_capacity(items.len());
    for item in *items {
        let Node::Map(m) = item else { return None };
        let mut parts = Vec::new();
        for f in fields {
            let Some(e) = m.entries.iter().find(|x| x.key == *f) else {
                continue;
            };
            let Node::Scalar { value, .. } = &e.value else {
                return None;
            };
            parts.push(format!("{f}={}", scalar_text(value)));
        }
        if parts.is_empty() {
            return None;
        }
        out.push(format!("[{}]", parts.join(",")));
    }
    // A duplicate key makes the list unkeyed; a set keeps the check linear.
    let unique: HashSet<&str> = out.iter().map(String::as_str).collect();
    (unique.len() == out.len()).then_some(out)
}

/// A work budget shared by the replays of one call; past it a replay fails and the caller falls back to a whole text.
pub struct Work {
    spent: Cell<u64>,
    limit: u64,
}

impl Work {
    pub fn unlimited() -> Self {
        Self::limited(u64::MAX)
    }

    pub fn limited(limit: u64) -> Self {
        Work {
            spent: Cell::new(0),
            limit,
        }
    }

    /// Charges `n` units; false once the budget is spent.
    pub fn spend(&self, n: usize) -> bool {
        let spent = self.spent.get().saturating_add(n as u64);
        self.spent.set(spent);
        spent <= self.limit
    }
}

/// A keyed list's segments and where each one sits.
pub struct Segs {
    pub names: Vec<String>,
    index: HashMap<String, usize>,
}

impl Segs {
    pub fn position(&self, seg: &str) -> Option<usize> {
        self.index.get(seg).copied()
    }
}

/// The segments of each list of one parsed tree, built once; a cache lives as long as its tree.
pub struct SegCache<'w> {
    work: &'w Work,
    lists: RefCell<HashMap<usize, Option<Rc<Segs>>>>,
}

impl<'w> SegCache<'w> {
    pub fn new(work: &'w Work) -> Self {
        SegCache {
            work,
            lists: RefCell::new(HashMap::new()),
        }
    }

    pub fn of(&self, name: Option<&str>, list: &Node) -> Option<Rc<Segs>> {
        let key = list as *const Node as usize;
        if let Some(hit) = self.lists.borrow().get(&key) {
            return hit.clone();
        }
        let built = item_segments(name, Some(list)).map(|names| {
            self.work.spend(names.len());
            let index = names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.clone(), i))
                .collect();
            Rc::new(Segs { names, index })
        });
        self.lists.borrow_mut().insert(key, built.clone());
        built
    }
}

/// Where a key path leads: the index path the document takes, as far as it resolves.
pub struct Loc {
    pub at: Vec<PathSeg>,
    /// The first segment that does not resolve; None when the whole path does.
    pub missing: Option<usize>,
    /// The path names an item, or a whole keyed list, that is gone: an op under it has nothing to land on.
    pub gone: bool,
}

pub fn locate(cache: &SegCache, root: &Node, path: &[&str]) -> Loc {
    let mut at = Vec::new();
    let mut node: &Node = root;
    for (i, seg) in path.iter().enumerate() {
        match node {
            Node::List { items, .. } => {
                let segs = cache.of(i.checked_sub(1).map(|j| path[j]), node);
                let idx = segs.as_ref().and_then(|s| s.position(seg));
                let Some(idx) = idx else {
                    let gone = segs.is_none() || i + 1 < path.len();
                    return Loc {
                        at,
                        missing: Some(i),
                        gone,
                    };
                };
                at.push(PathSeg::Index(idx));
                node = &items[idx];
            }
            Node::Map(m) if m.entries.iter().any(|e| e.key == *seg) => {
                at.push(PathSeg::Key((*seg).to_owned()));
                node = &m
                    .entries
                    .iter()
                    .find(|e| e.key == *seg)
                    .expect("found")
                    .value;
            }
            _ => {
                return Loc {
                    at,
                    missing: Some(i),
                    gone: !matches!(node, Node::Map(_)),
                };
            }
        }
    }
    Loc {
        at,
        missing: None,
        gone: false,
    }
}

/// The value at `path` in a parsed k7x text as the ops carry it, or None when absent.
pub fn value_at(cache: &SegCache, text: &str, root: &Node, path: &[&str]) -> Option<String> {
    if path.is_empty() {
        return Some(text.to_owned());
    }
    let loc = locate(cache, root, path);
    if loc.missing.is_some() {
        return None;
    }
    root.at(&loc.at).map(|n| fragment(text, n).to_owned())
}

/// True when the path names an item, or a whole keyed list, that is gone.
pub fn gone_at(cache: &SegCache, root: &Node, path: &[&str]) -> bool {
    locate(cache, root, path).gone
}
