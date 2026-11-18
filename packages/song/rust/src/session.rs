// session.rs — one workspace's compile as a host calls it: a Cache and the answers it made, over any Source, with no host in sight.
// A reader would get wrong that `rev` keys anything: it only spares the stat of a read set, and an answer is keyed by the request.

mod make;

use crate::cache::Cache;
use crate::clips::nest_message;
use crate::doc::Vsts;
use crate::files::Severity;
use crate::graph::meter::has_zero_half;
use crate::graph::{Folder, GraphError, ModelNeed, model_need};
use crate::modules::Vocabulary;
use crate::readset::ReadSet;
use crate::record::{Extras, Recorder};
use crate::roots::Roots;
use crate::source::path::PathRule;
use crate::source::{BundleRequest, Source, SourceBundle, SourceError};
use make::{Core, Group, Made};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

/// How many answers a session holds, the least recently asked dropped (decision 707).
pub const MAX_ANSWERS: usize = 4;

/// One thing a host asks of a project; `Engine`, `Web` and `Errors` come from one compile and are always made together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum View {
    Engine,
    Web,
    Errors,
    Hints,
    Surfaces,
    IoPorts,
    Catalogue,
    BaseModules,
    Graph,
}

/// What a host asks: where to read, the rig, the engine's VSTs, and what the host knows that the files do not say.
#[derive(Clone, Debug)]
pub struct Ask {
    pub roots: Roots,
    pub rig: serde_json::Value,
    pub vsts: Vsts,
    pub thawed: Vec<String>,
    /// A token the host's own revision memo gives; equal tokens on one request mean nothing moved (decision 706).
    pub rev: Option<String>,
    /// Which folder a graph is drawn for: the open project, or any other (a library chain).
    pub folder: Folder,
}

/// The answers to one ask, in the order of the views asked, and what the compile found beside them.
#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub views: Vec<(View, Arc<str>)>,
    pub renders_used: Vec<String>,
    /// Whether every view came from a held answer.
    pub reused: bool,
}

/// What a host's comparison needs of the project that no view says (decisions 454 and 518).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The time signature has a half of zero.
    pub zero_meter: bool,
    /// The compile cut a nest of timelines at its depth.
    pub nested_cut: bool,
    /// A clip or sequence file that does not decode was read past (decision 293).
    pub broken_material: bool,
}

/// The `doc` slice of an engine view `{"doc":…,"freezes":…}`, cut at the doc's own closing brace without a parse (decisions 1119, 1518, 1523).
pub fn engine_doc_of(text: &str) -> Option<&str> {
    const HEAD: &str = "{\"doc\":";
    const FREEZES: &str = ",\"freezes\":[";
    let body = text.strip_prefix(HEAD)?;
    let end = value_end(body.as_bytes())?;
    let (doc, rest) = body.split_at(end);
    (doc.starts_with('{') && rest.starts_with(FREEZES) && rest.ends_with('}')).then_some(doc)
}

/// The end of the object or array opening `b`: the scan skips strings and counts depth, so a key a user wrote cannot move it.
fn value_end(b: &[u8]) -> Option<usize> {
    let (mut depth, mut i) = (0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ if depth == 0 => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

type Key = [u8; 16];

/// The key of a request: everything that can change an answer but the files themselves.
fn key_of(ask: &Ask) -> Key {
    let text = serde_json::to_string(&(&ask.roots, &ask.rig, ask.vsts, &ask.thawed, ask.folder == Folder::Open))
        .unwrap_or_default();
    let sum = Sha256::digest(text.as_bytes());
    let mut key = [0u8; 16];
    key.copy_from_slice(&sum[..16]);
    key
}

struct Held {
    key: Key,
    tick: u64,
    core: Option<Core>,
    others: BTreeMap<Group, Made>,
}

impl Held {
    fn made(&self, group: Group) -> Option<&Made> {
        match group {
            Group::Core => self.core.as_ref().map(|c| &c.made),
            g => self.others.get(&g),
        }
    }

    fn made_mut(&mut self, group: Group) -> Option<&mut Made> {
        match group {
            Group::Core => self.core.as_mut().map(|c| &mut c.made),
            g => self.others.get_mut(&g),
        }
    }

    /// Whether the held answer of a group still stands: the host's token equal, else its read set unmoved.
    fn fresh(&mut self, group: Group, source: &dyn Source, rev: Option<&str>) -> bool {
        let Some(made) = self.made_mut(group) else { return false };
        if rev.is_some() && made.rev.as_deref() == rev {
            return true;
        }
        if made.set.settled() && made.set.valid(source) {
            made.rev = rev.map(str::to_owned);
            return true;
        }
        false
    }

    fn weight(&self) -> usize {
        let core = self.core.as_ref().map_or(0, |c| c.made.weight() + c.compiled_weight());
        core + self.others.values().map(Made::weight).sum::<usize>()
    }
}

/// One workspace's cache and answers; a host holds one per workspace.
pub struct Session {
    cache: Cache,
    held: Vec<Held>,
    tick: u64,
    serials: u64,
}

impl Session {
    pub fn new(rule: PathRule) -> Session {
        Session {
            cache: Cache::with_paths(rule),
            held: Vec::new(),
            tick: 0,
            serials: 0,
        }
    }

    /// The held answer for a request, made when none is, the oldest dropped to make room.
    fn slot(&mut self, key: Key) -> usize {
        self.tick += 1;
        if let Some(i) = self.held.iter().position(|h| h.key == key) {
            self.held[i].tick = self.tick;
            return i;
        }
        if self.held.len() >= MAX_ANSWERS
            && let Some(oldest) = (0..self.held.len()).min_by_key(|i| self.held[*i].tick)
        {
            self.held.remove(oldest);
        }
        self.held.push(Held {
            key,
            tick: self.tick,
            core: None,
            others: BTreeMap::new(),
        });
        self.held.len() - 1
    }

    /// Makes the group unless a held one stands; whether it was held. `Missing` is the Err alone.
    fn ensure(&mut self, at: usize, group: Group, source: &dyn Source, ask: &Ask) -> Result<bool, SourceError> {
        if group == Group::Graph {
            // A graph is drawn from the compile, so it stands only over the very compile it was drawn over.
            let core_held = self.ensure(at, Group::Core, source, ask)?;
            let over = self.held[at].core.as_ref().map(|c| c.serial);
            let drawn_over = self.held[at].others.get(&group).is_some_and(|m| m.over == over);
            if core_held && drawn_over && self.held[at].fresh(group, source, ask.rev.as_deref()) {
                return Ok(true);
            }
            let made = make::other(&mut self.cache, source, ask, group, self.held[at].core.as_ref())?;
            self.held[at].others.insert(group, made);
            return Ok(false);
        }
        if self.held[at].fresh(group, source, ask.rev.as_deref()) {
            return Ok(true);
        }
        if group == Group::Core {
            let mut core = make::core(&mut self.cache, source, ask)?;
            self.serials += 1;
            core.serial = self.serials;
            self.held[at].core = Some(core);
        } else {
            let made = make::other(&mut self.cache, source, ask, group, None)?;
            self.held[at].others.insert(group, made);
        }
        Ok(false)
    }

    /// The views asked, each from a held answer while it stands and made otherwise (decisions 705 to 707).
    pub fn answer(&mut self, source: &dyn Source, ask: &Ask, views: &[View]) -> Result<Reply, SourceError> {
        self.cache.set_paths(ask.roots.paths);
        let at = self.slot(key_of(ask));
        let mut reply = Reply {
            reused: true,
            ..Reply::default()
        };
        for &view in views {
            let group = Group::of(view);
            reply.reused &= self.ensure(at, group, source, ask)?;
            if let Some(text) = self.held[at].made(group).and_then(|m| m.text(view)) {
                reply.views.push((view, text));
            }
        }
        if let Some(core) = &self.held[at].core
            && views
                .iter()
                .any(|v| matches!(Group::of(*v), Group::Core | Group::Graph))
        {
            reply.renders_used = core.used.clone();
        }
        Ok(reply)
    }

    /// The bundle of the compile of this request: its reads, the digests, what was stale and the lengths measured (decision 751).
    pub fn bundle(&mut self, source: &dyn Source, ask: &Ask) -> Result<SourceBundle, SourceError> {
        self.cache.set_paths(ask.roots.paths);
        let rec = Recorder::new(source);
        let core = make::core(&mut self.cache, &rec, ask)?;
        let (dirs, files) = ask.roots.mounts()?;
        let extras = Extras {
            jsdsp_stale: Default::default(),
            lengths: core.lengths.clone(),
            request: Some(BundleRequest {
                roots: ask.roots.clone(),
                rig: ask.rig.clone(),
                vsts: match ask.vsts {
                    Vsts::Scan => "scan".to_owned(),
                    Vsts::None => "none".to_owned(),
                },
                thawed: ask.thawed.clone(),
            }),
        };
        let bundle = rec.into_bundle_with(&dirs, &files, extras);
        match &bundle.overflow {
            Some(why) => Err(SourceError::Io(why.clone())),
            None => Ok(bundle),
        }
    }

    /// Whether anything the held answer of `view` read has an mtime later than `since_ns` (decision 717).
    pub fn moved_since(&self, source: &dyn Source, ask: &Ask, view: View, since_ns: u64) -> bool {
        let key = key_of(ask);
        let Some(held) = self.held.iter().find(|h| h.key == key) else {
            return false;
        };
        held.made(Group::of(view))
            .is_some_and(|m: &Made| ReadSet::moved_after(&m.set, source, since_ns))
    }

    /// What the project says of itself that a comparison of the views needs; a file that will not read says nothing.
    pub fn facts(&mut self, source: &dyn Source, ask: &Ask) -> Facts {
        let key = key_of(ask);
        let nested = nest_message();
        let nested_cut = self
            .held
            .iter()
            .find(|h| h.key == key)
            .and_then(|h| h.core.as_ref())
            .is_some_and(|c| c.compiled.diagnostics.iter().any(|f| f.message == nested));
        let broken_material = self
            .held
            .iter()
            .find(|h| h.key == key)
            .and_then(|h| h.core.as_ref())
            .is_some_and(|c| {
                c.compiled.diagnostics.iter().any(|f| {
                    f.severity == Severity::Error && (f.file.starts_with("clips/") || f.file.starts_with("sequences/"))
                })
            });
        let zero_meter = crate::roots::spell(&ask.roots.project, ask.roots.paths)
            .ok()
            .and_then(|p| has_zero_half(source, &mut self.cache, &p).ok())
            .unwrap_or(false);
        Facts {
            zero_meter,
            nested_cut,
            broken_material,
        }
    }

    /// The module vocabulary under `roots`, held while what it read has not moved (1p-D2).
    pub fn vocabulary(&mut self, source: &dyn Source, roots: &Roots) -> Result<Arc<Vocabulary>, SourceError> {
        self.cache.set_paths(roots.paths);
        self.cache.vocabulary(source, roots)
    }

    /// How many vocabulary asks the cache answered from what it held.
    pub fn vocabulary_hits(&self) -> usize {
        self.cache.vocabulary_hits()
    }

    /// The model a node names and the machine lacks (2446): read off the graph, held nowhere, so a model that arrived is seen at once.
    pub fn model_need(&mut self, source: &dyn Source, ask: &Ask, node: &str) -> Result<ModelNeed, GraphError> {
        self.cache.set_paths(ask.roots.paths);
        model_need(source, &mut self.cache, &ask.roots, make::request_of(ask).rig, node)
    }

    /// How many answers are held, never more than `MAX_ANSWERS`.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Whether the held answer of `view` read a path the Source refused (decision 722).
    pub fn refused(&self, ask: &Ask, view: View) -> bool {
        let key = key_of(ask);
        self.held
            .iter()
            .find(|h| h.key == key)
            .and_then(|h| h.made(Group::of(view)))
            .is_some_and(|m| m.set.has_refused())
    }

    /// What the session holds: the cache's texts and the answers it keeps.
    pub fn bytes(&self) -> usize {
        self.cache.bytes() + self.held.iter().map(Held::weight).sum::<usize>()
    }

    /// Runs one of the send-reference ports (`send_refs`) over this session's cache, which a host cannot otherwise reach.
    pub fn with_cache<R>(&mut self, paths: crate::PathRule, f: impl FnOnce(&mut Cache) -> R) -> R {
        self.cache.set_paths(paths);
        f(&mut self.cache)
    }

    /// Lets go of every answer and every decoded file.
    pub fn forget(&mut self) {
        self.held.clear();
        self.cache.forget();
    }

    /// Lets go of the answers and most of the cache, as a host short of memory does.
    pub fn shed(&mut self) {
        self.held.clear();
        self.cache.shed();
    }
}

#[cfg(test)]
mod tests {
    use super::engine_doc_of;

    #[test]
    fn the_doc_is_cut_at_its_own_closing_brace() {
        let doc = r#"{"a":",\"freezes\":[1]","b":{"freezes":2}}"#;
        let text = format!(r#"{{"doc":{doc},"freezes":[{{"plan":1}}]}}"#);
        assert_eq!(engine_doc_of(&text), Some(doc));
    }

    #[test]
    fn a_freezes_key_inside_a_freezes_cut_is_the_docs_own_end_not_a_cut() {
        let cut = r#"{"plugins":[{"a":1,"freezes":[1]}],"freezes":[{"cut":{"freezes":[2]}}]}"#;
        let text = format!(r#"{{"doc":{{"tracks":[]}},"freezes":[{{"cut":{cut}}}]}}"#);
        assert_eq!(engine_doc_of(&text), Some(r#"{"tracks":[]}"#));
        let inner = r#"{"tracks":[{"a":1,"freezes":[1],"s":"\\","t":"\"}"}],"freezes":[3]}"#;
        let text = format!(r#"{{"doc":{inner},"freezes":[]}}"#);
        assert_eq!(engine_doc_of(&text), Some(inner));
    }

    #[test]
    fn what_is_not_an_engine_view_is_no_doc() {
        for text in [
            "",
            "{}",
            r#"{"failure":{"diagnostics":[]}}"#,
            r#"{"doc":{},"other":[]}"#,
            r#"{"doc":{},"freezes":{}}"#,
            r#"{"doc":{},"freezes":[]"#,
        ] {
            assert_eq!(engine_doc_of(text), None, "{text}");
        }
        assert_eq!(engine_doc_of(r#"{"doc":{},"freezes":[]}"#), Some("{}"));
    }

    #[test]
    fn a_large_doc_is_sliced_not_copied() {
        let doc = format!("{{\"x\":\"{}\"}}", "a".repeat(31 << 20));
        let text = format!(r#"{{"doc":{doc},"freezes":[]}}"#);
        let cut = engine_doc_of(&text).expect("a slice");
        assert_eq!(cut.len(), doc.len());
        assert!(std::ptr::eq(cut.as_ptr(), text[7..].as_ptr()));
    }
}
