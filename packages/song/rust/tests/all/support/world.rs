// support/world.rs — a project read and resolved as the tests need it: worlds on disk and in memory, the module half's dump.
// A reader would get wrong that nothing here is a test: it is what the stage gate and the negative corpus share.

#![allow(dead_code)]

use super::*;
use std::sync::Arc;

use song::modules::base_module_refs;

/// What the module half of a project dump holds, counted for the stage gate's minimums.
#[derive(Default, Clone, Copy)]
pub struct ModCounts {
    pub nodes: usize,
    pub library_findings: usize,
    pub scan_plugins: usize,
}

pub struct ModDump {
    pub json: Value,
    pub counts: ModCounts,
    pub chain: bool,
}

fn files_of<'a>(findings: impl Iterator<Item = &'a song::Finding>) -> Vec<String> {
    let mut files: Vec<String> = findings.map(|f| f.file.clone()).collect();
    files.sort();
    files.dedup();
    files
}

/// Each finding's file with its severity, sorted: `<file>|<severity>`.
fn severities(findings: &[song::Finding]) -> Vec<String> {
    let mut out: Vec<String> = findings
        .iter()
        .map(|f| {
            format!(
                "{}|{}",
                f.file,
                if f.severity == song::Severity::Warning {
                    "warning"
                } else {
                    "error"
                }
            )
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The refs whose module folder holds `<base>.k7x`, as the TypeScript's sibling walk answers it.
fn siblings(source: &dyn Source, v: &song::Vocabulary, base: &str) -> Result<Vec<String>, SourceError> {
    let mut refs = Vec::new();
    for f in &v.folders {
        let path = format!("{}/{base}.k7x", f.path);
        match source.stat(&path) {
            Ok(st) if st.kind == song::Kind::File => refs.push(f.module_ref.clone()),
            Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => {}
            Err(e) => return Err(e),
        }
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}

/// The module half of a project as the Rust reads it, in the shape `scripts/stageDump.ts` writes for the TypeScript.
pub fn dump_modules(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<ModDump, SourceError> {
    let w = world(source, cache, roots)?;
    let (nodes, vocab) = (&w.nodes, &w.vocab);
    let value = |m: &song::LoadedModule| serde_json::to_value(m).expect("json");
    let node_values: Map<String, Value> = nodes.nodes.iter().map(|(k, m)| (k.clone(), value(m))).collect();
    let modules: Map<String, Value> = vocab.modules.iter().map(|(k, m)| (k.clone(), value(m))).collect();
    let resolved = |r: &song::Resolution| -> Map<String, Value> {
        r.modules
            .iter()
            .map(|(k, m)| (k.clone(), serde_json::to_value(m).expect("json")))
            .collect()
    };
    let counts = ModCounts {
        nodes: nodes.nodes.len(),
        library_findings: vocab
            .findings
            .iter()
            .filter(|f| f.file.ends_with("/module.k7x"))
            .count(),
        scan_plugins: vocab.scan.plugins.len(),
    };
    let plugin = song::resolve::plugin_findings(&nodes.nodes, &w.resolved.modules);
    // Ids as the gate counts them: a pick other than the first, and an `extends` some module answers to.
    let ids = |keep: &dyn Fn(&song::LoadedModule) -> bool| -> Vec<String> {
        let n = nodes.nodes.iter().map(|(k, m)| (format!("n:{k}"), m));
        let v = vocab.modules.iter().map(|(k, m)| (format!("v:{k}"), m));
        n.chain(v).filter(|(_, m)| keep(m)).map(|(id, _)| id).collect()
    };
    let picked = ids(&|m| song::resolve::current_alternative(&m.authored) != 0);
    let folds = ids(&|m| {
        m.authored
            .extends
            .as_deref()
            .is_some_and(|e| vocab.modules.contains_key(e))
    });
    let mut absent = Map::new();
    for m in nodes.nodes.values().chain(vocab.modules.values()) {
        if let Some(r) = m
            .authored
            .extends
            .as_deref()
            .filter(|r| !vocab.modules.contains_key(*r))
        {
            let why = match vocab.absent(r) {
                song::modules::Absent::Unknown => "Unknown".to_owned(),
                song::modules::Absent::Broken { file } => file,
            };
            absent.insert(r.to_owned(), json!(why));
        }
    }
    let routing = routing_json(source, cache, roots, &w)?;
    let mut labels: Vec<String> = nodes.nodes.values().map(song::LoadedModule::own_label).collect();
    labels.sort();
    labels.dedup();
    let json = json!({
        "routing": routing,
        "nodes": { "values": node_values, "files": files_of(nodes.findings.iter()), "labels": labels },
        "vocab": {
            "modules": modules,
            "findings": files_of(vocab.findings.iter()),
            "findings_severity": severities(&vocab.findings),
            "base": base_module_refs(&vocab.modules),
            "siblings": { "ui": siblings(source, vocab, "ui")?, "controller": siblings(source, vocab, "controller")? },
        },
        "resolved": {
            "nodes": resolved(&w.resolved),
            "vocab": resolved(&w.resolved_vocab),
            "plugin": severities(&plugin),
        },
        "resolve": {
            "findings": severities(&w.resolved.findings),
            "vocab_findings": severities(&w.resolved_vocab.findings),
            "node_warnings": severities(&nodes.warnings),
            "panels": vocab.panels.ui.keys().collect::<Vec<_>>(),
            "maps": vocab.panels.maps.keys().collect::<Vec<_>>(),
            "absent": absent,
            "picked": picked,
            "folds": folds,
            "weights": [w.resolved.weight, w.resolved_vocab.weight],
        },
        "scan_builtin": vocab.scan.is_empty(),
    });
    Ok(ModDump {
        json,
        counts,
        chain: nodes.chain_project,
    })
}

/// The routing half of a project: its segments, levels and derived chains, and what the read found.
fn routing_json(source: &dyn Source, cache: &mut Cache, roots: &Roots, w: &World) -> Result<Value, SourceError> {
    use song::routing::{
        apply_link_levels, derive_chains, lane_names, load_routing, tap_pairs, with_chain_module_refs,
    };
    let r = load_routing(source, cache, roots)?;
    let lanes = lane_names(&w.resolved.modules);
    let mut derived = derive_chains(&r.segments, &|n| lanes.contains(n));
    apply_link_levels(&mut derived, &r.levels);
    let refs = with_chain_module_refs(&w.nodes.nodes, &w.vocab.modules, &derived);
    let modules_map = song::resolve_map(source, roots, &refs, &w.vocab.modules)?;
    let mut lane_list: Vec<&String> = lanes.iter().collect();
    lane_list.sort();
    let pairs: Map<String, Value> = tap_pairs(r.segments.tap_feeds_of())
        .iter()
        .map(|(n, l)| {
            (
                n.clone(),
                json!(
                    l.iter()
                        .map(|t| json!({ "tap": t.tap, "pair": t.pair }))
                        .collect::<Vec<_>>()
                ),
            )
        })
        .collect();
    let project = r.project.as_ref();
    let nodes: Map<String, Value> = project
        .into_iter()
        .flat_map(|p| p.nodes.values())
        .map(|n| {
            let mut v = json!({ "id": n.id, "chain": n.chain, "step": n.step, "item": n.item });
            let o = v.as_object_mut().expect("an object");
            for (k, x) in [("sub", n.sub.as_ref()), ("track", n.track.as_ref())] {
                if let Some(x) = x {
                    o.insert(k.to_owned(), json!(x));
                }
            }
            if n.picture == Some(true) {
                o.insert("picture".to_owned(), json!(true));
            }
            (n.name.clone(), v)
        })
        .collect();
    Ok(json!({
        "chain_project": r.chain_project,
        "segments": r.segments,
        "levels": r.levels,
        "derived": derived,
        "bypassed": r.segments.bypassed_of().collect::<Vec<_>>(),
        "bus_only": r.segments.bus_only_of().collect::<Vec<_>>(),
        "tap_pairs": pairs,
        "lanes": lane_list,
        "files": severities(&r.findings),
        "capped": r.capped,
        "weight": r.weight,
        "chains": project.map(|p| p.chains.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "hidden": project.map(|p| p.hidden.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "nodes": nodes,
        "modules_map": modules_map.modules.iter().map(|(k, m)| (k.clone(), serde_json::to_value(m).expect("json"))).collect::<Map<String, Value>>(),
    }))
}

/// The sub-step a stripped key waits for must come AFTER the crate's own stage: a list that only grows goes stale unseen.
pub fn strip_checked(spec: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in spec.as_array().into_iter().flatten() {
        let (key, step) = (
            entry["key"].as_str().unwrap_or(""),
            entry["step"].as_str().unwrap_or(""),
        );
        assert!(
            !key.is_empty() && step > song::STAGE,
            "a stripped key waits for a later sub-step: {entry}"
        );
        out.push((key.to_owned(), step.to_owned()));
    }
    out
}

/// Drops the keys a later sub-step fills, as the dump's own `strip` names them (empty from 2c on), from one module.
pub fn strip_module(m: &mut Value, spec: &[(String, String)]) {
    if let Some(obj) = m.as_object_mut() {
        for (key, _) in spec {
            obj.remove(key);
        }
    }
}

use indexmap::IndexMap;
use song::modules::{LoadedModule, ModuleKind, decode_module};
use song::{Entry as SrcEntry, Resolution, Span, Stat};
use std::sync::Mutex;

/// The roots of a memory world: everything lives under `/p`, as `mem` lays it out.
pub fn mem_world_roots() -> Roots {
    Roots {
        project: "/p/ws/proj".into(),
        workspace: Some("/p/ws".into()),
        libraries: vec![
            Library {
                name: "account".into(),
                root: "/p/lib/account".into(),
                editable: true,
            },
            Library {
                name: "k7-core".into(),
                root: "/p/lib/k7-core".into(),
                editable: false,
            },
        ],
        meta: Some("/p/meta".into()),
        models: Some("/p/models".into()),
        confine: Some("/p".into()),
        ..Roots::default()
    }
}

/// A module decoded from its text and wrapped as a loaded one of `category`, with `file` as its folder's name.
pub fn module_of(category: &str, file: &str, text: &str) -> LoadedModule {
    let m = decode_module("m.k7x", text).unwrap_or_else(|f| panic!("{text}: {}", f.message));
    let mut lm = LoadedModule::bare(ModuleKind::Plugin, category, file);
    lm.authored = m;
    lm
}

/// A project node by hand: no type of its own (an `extends` gives it one), filed under `nodes/`.
pub fn node_of(file: &str, text: &str) -> LoadedModule {
    let mut lm = module_of("", file, text);
    lm.kind = None;
    lm.category = None;
    lm.home = Some("nodes".into());
    lm.name = Some(file.to_owned());
    lm.node = Some(file.to_owned());
    lm
}

/// A vocabulary by hand: ref to module text, each a plugin of the `audio` category.
pub fn by_ref(entries: &[(&str, &str)]) -> IndexMap<String, LoadedModule> {
    entries
        .iter()
        .map(|(r, text)| {
            (
                (*r).to_owned(),
                module_of("audio", r.rsplit('/').next().unwrap_or(r), text),
            )
        })
        .collect()
}

/// Resolves `modules` against `by_ref` over an empty memory world.
pub fn resolve(modules: &IndexMap<String, LoadedModule>, by_ref: &IndexMap<String, LoadedModule>) -> Resolution {
    let src = mem(&[]);
    song::resolve_map(&src, &mem_world_roots(), modules, by_ref).expect("resolution")
}

/// One module resolved by hand against a vocabulary by hand.
pub fn resolve_one(text: &str, entries: &[(&str, &str)]) -> song::Resolved {
    let refs = by_ref(entries);
    let node: IndexMap<String, LoadedModule> = [("n".to_owned(), node_of("n", text))].into_iter().collect();
    let mut out = resolve(&node, &refs);
    out.modules.shift_remove("n").expect("resolved")
}

/// A Source that remembers every read it served, to prove what is only ever stat'ed.
pub struct Spy<'a> {
    pub inner: &'a dyn Source,
    pub reads: Mutex<Vec<String>>,
}

impl Source for Spy<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<SrcEntry>, SourceError> {
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.reads.lock().expect("lock").push(path.to_owned());
        self.inner.read(path, span)
    }
}

/// Whether the temporary folder folds case: `a` and `A` are one file there.
pub fn folds_case(t: &Tmp) -> bool {
    t.write("CaseProbe", "x");
    t.0.join("caseprobe").exists()
}

use song::modules::{NodesRead, Vocabulary};

/// A project read and resolved: its vocabulary, its nodes, and both resolved.
pub struct World {
    pub vocab: Arc<Vocabulary>,
    pub nodes: NodesRead,
    pub resolved: Resolution,
    pub resolved_vocab: Resolution,
}

/// Loads and resolves everything `roots` reaches through `source`, with the cache given.
pub fn world(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<World, SourceError> {
    let nodes = song::modules::load_nodes(source, cache, roots)?;
    let vocab = cache.vocabulary(source, roots)?;
    let resolved = song::resolve_map(source, roots, &nodes.nodes, &vocab.modules)?;
    let resolved_vocab = song::resolve_map(source, roots, &vocab.modules, &vocab.modules)?;
    Ok(World {
        vocab,
        nodes,
        resolved,
        resolved_vocab,
    })
}

/// A disk world: the project's, the account's and k7-core's files written under a temporary folder and aged.
pub fn disk_world(t: &Tmp, files: &[(&str, &str)]) -> World {
    for (rel, text) in files {
        t.write(rel, text);
    }
    t.mkdir("ws/proj");
    t.age();
    let r = roots(t);
    world(&disk(&r), &mut Cache::new(), &r).expect("a world")
}

/// The same files in memory, under `/p`, read with the roots of `mem_world_roots`.
pub fn mem_world(files: &[(&str, &str)]) -> World {
    world(&mem(files), &mut Cache::new(), &mem_world_roots()).expect("a world")
}
