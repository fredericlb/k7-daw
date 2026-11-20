// freeze_support/mod.rs — what the freeze tests share: chains, modules and documents built by hand, and a timed run.
// A reader would get wrong that nothing here reads a project: a case states only the graph and the document it tests.

#![allow(dead_code)]

use indexmap::IndexMap;
use song::doc::{PluginDoc, ProjectDoc, SendDoc, TrackDoc};
use song::modules::{ModuleKind, Plugin};
use song::routing::{Chain, Out, UseStep};
use song::{LoadedModule, Module};
use std::time::{Duration, Instant};

/// A run: the lane or chain it starts at, the modules it passes through, where it goes.
pub fn chain(from: Option<&str>, uses: &[&str], outs: &[&str]) -> Chain {
    Chain {
        from: from.map(str::to_owned),
        from_port: None,
        use_steps: uses
            .iter()
            .map(|m| UseStep {
                module: (*m).to_owned(),
                inputs: None,
                enabled: None,
            })
            .collect(),
        out: outs
            .iter()
            .map(|to| Out {
                to: (*to).to_owned(),
                gain: 1.0,
                from_port: None,
            })
            .collect(),
    }
}

pub fn chains(items: Vec<(&str, Chain)>) -> IndexMap<String, Chain> {
    items.into_iter().map(|(k, c)| (k.to_owned(), c)).collect()
}

/// A plugin module of a format and a name, as a scan or a module file would make one.
pub fn plugin_module(format: &str, name: &str) -> LoadedModule {
    let mut m = LoadedModule::bare(ModuleKind::Plugin, "audio", name);
    m.authored = Module {
        plugin: Some(Plugin {
            plugin_type: Some(format.to_owned()),
            name: Some(name.to_owned()),
            ..Plugin::default()
        }),
        ..Module::default()
    };
    m
}

pub fn vst(name: &str) -> LoadedModule {
    plugin_module("vst3", name)
}

/// A track playing `modules`, each as the plugin `<track>:<module>`.
pub fn track(id: &str, modules: &[&str]) -> TrackDoc {
    let mut t = TrackDoc::new(id);
    t.plugins = modules.iter().map(|m| PluginDoc::new(&format!("{id}:{m}"))).collect();
    t
}

pub fn send(to: &str) -> SendDoc {
    SendDoc {
        to: to.to_owned(),
        level: 1.0,
        enabled: true,
    }
}

pub fn doc(tracks: Vec<TrackDoc>) -> ProjectDoc {
    ProjectDoc {
        tracks,
        ..ProjectDoc::empty()
    }
}

/// The bound of a hostile case: five seconds, thirty on Alpine, whose allocator is far slower.
pub const BOUND_SECS: u64 = if cfg!(target_env = "musl") { 30 } else { 5 };

/// Runs `f` and fails when it took the bound or more.
pub fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(
        t.elapsed() < Duration::from_secs(BOUND_SECS),
        "{what} took {:?}",
        t.elapsed()
    );
    out
}

pub fn sorted<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = items.into_iter().map(str::to_owned).collect();
    v.sort();
    v
}

use song::{Entry, Kind, Source, SourceError, Span, Stamp, Stat};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A Source that counts the stats and reads it forwards.
pub struct Counting<'a> {
    pub inner: &'a dyn Source,
    pub stats: AtomicUsize,
    pub reads: AtomicUsize,
}

impl<'a> Counting<'a> {
    pub fn new(inner: &'a dyn Source) -> Counting<'a> {
        Counting {
            inner,
            stats: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
        }
    }

    pub fn stats(&self) -> usize {
        self.stats.load(Ordering::SeqCst)
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl Source for Counting<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.stats.fetch_add(1, Ordering::SeqCst);
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read(path, span)
    }
}

/// A Source with no file at all, which counts the stats it is asked.
pub struct Nothing {
    pub stats: AtomicUsize,
}

impl Nothing {
    pub fn new() -> Nothing {
        Nothing {
            stats: AtomicUsize::new(0),
        }
    }
}

impl Source for Nothing {
    fn stat(&self, _path: &str) -> Result<Stat, SourceError> {
        self.stats.fetch_add(1, Ordering::SeqCst);
        Err(SourceError::NotFound)
    }

    fn list(&self, _dir: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::NotFound)
    }

    fn read(&self, _path: &str, _span: Span) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::NotFound)
    }
}

/// Every file under `/s/` is `len` bytes of `a`, never settled; `vanish` makes a read answer NotFound after a good stat.
pub struct Synthetic {
    pub len: u64,
    pub vanish: bool,
    pub stats: AtomicUsize,
    pub reads: AtomicUsize,
}

impl Synthetic {
    pub fn new(len: u64) -> Synthetic {
        Synthetic {
            len,
            vanish: false,
            stats: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
        }
    }
}

impl Source for Synthetic {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.stats.fetch_add(1, Ordering::SeqCst);
        if !path.starts_with("/s/") {
            return Err(SourceError::Outside);
        }
        Ok(Stat {
            kind: Kind::File,
            len: self.len,
            stamp: Stamp {
                mtime_ns: 1,
                len: self.len,
                settled: false,
            },
        })
    }

    fn list(&self, _dir: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::NotFound)
    }

    fn read(&self, _path: &str, _span: Span) -> Result<Vec<u8>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.vanish {
            return Err(SourceError::NotFound);
        }
        Ok(vec![b'a'; usize::try_from(self.len).unwrap_or(0)])
    }
}

/// A document whose one plugin names `path` as its surge patch, the field a fingerprint walks for files.
pub fn naming(path: &str) -> ProjectDoc {
    let mut t = track("gtr", &["amp"]);
    t.plugins[0].surge_patch = Some(path.to_owned());
    doc(vec![t])
}

/// The first 16 hex of the SHA-256 of `bytes`, as a token spells a digest.
pub fn hex16(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

use song::Cache;
use song::freeze::{AutoFreezes, AutoInput, Edges, FreezePlan, auto_freeze_points, auto_freezes};
use song::modules::{Scan, ScannedPlugin};
use song::routing::Chain as RoutedChain;
use std::collections::HashSet;

/// A scan naming these `(format, name)` plugins.
pub fn scan_of(plugins: &[(&str, &str)]) -> Scan {
    Scan::from_plugins(
        plugins
            .iter()
            .map(|(format, name)| ScannedPlugin {
                name: (*name).to_owned(),
                format: (*format).to_owned(),
                id: None,
                manufacturer: None,
                role: None,
                instrument: false,
                category: None,
            })
            .collect(),
    )
}

/// Everything the automatic freezes read, owned, so a case changes one part and runs.
pub struct Scene {
    pub project: String,
    pub renders: Option<String>,
    pub chains: IndexMap<String, RoutedChain>,
    pub modules: IndexMap<String, LoadedModule>,
    pub lanes: HashSet<String>,
    pub strudel: HashSet<String>,
    pub manual: Vec<FreezePlan>,
    pub thawed: HashSet<String>,
    pub scan: Scan,
    pub no_vsts: bool,
    pub live: ProjectDoc,
}

impl Scene {
    /// gtr -> amp (VST) -> cab -> OUT, keys -> verb (VST) -> OUT; `cab` a VST too when asked.
    pub fn standard(project: &str, renders: Option<&str>, cab_vst: bool) -> Scene {
        let mut modules = IndexMap::new();
        modules.insert("amp".to_owned(), vst("Amp"));
        modules.insert(
            "cab".to_owned(),
            if cab_vst {
                vst("Cab")
            } else {
                plugin_module("internal", "volume")
            },
        );
        modules.insert("verb".to_owned(), vst("Verb"));
        Scene {
            project: project.to_owned(),
            renders: renders.map(str::to_owned),
            chains: chains(vec![
                ("gtr", chain(Some("gtr"), &["amp", "cab"], &["OUT"])),
                ("keys", chain(Some("keys"), &["verb"], &["OUT"])),
            ]),
            modules,
            lanes: ["gtr", "keys"].iter().map(|s| (*s).to_owned()).collect(),
            strudel: HashSet::new(),
            manual: Vec::new(),
            thawed: HashSet::new(),
            scan: Scan::default(),
            no_vsts: false,
            live: doc(vec![track("gtr", &["amp", "cab"]), track("keys", &["verb"])]),
        }
    }

    fn with_input<T>(&self, f: impl FnOnce(&AutoInput<'_>) -> T) -> T {
        let edges = Edges::of(&self.chains);
        let names: Vec<&str> = self.modules.keys().map(String::as_str).collect();
        let module = |n: &str| self.modules.get(n);
        let is_lane = |n: &str| self.lanes.contains(n);
        let is_strudel = |n: &str| self.strudel.contains(n);
        let input = AutoInput {
            project: &self.project,
            renders: self.renders.as_deref(),
            edges: &edges,
            names: &names,
            module: &module,
            is_lane: &is_lane,
            is_strudel: &is_strudel,
            manual: &self.manual,
            thawed: &self.thawed,
            scan: &self.scan,
            no_vsts: self.no_vsts,
            live: &self.live,
            rule: song::PathRule::HOST,
        };
        f(&input)
    }

    pub fn run(&self, source: &dyn Source, cache: &mut Cache, budget: &mut song::clips::Budget) -> AutoFreezes {
        self.with_input(|i| auto_freezes(source, cache, i, budget))
            .expect("a source")
    }

    pub fn points(&self, budget: &mut song::clips::Budget) -> Option<Vec<String>> {
        self.with_input(|i| auto_freeze_points(i, budget))
    }

    pub fn modules_of(&self, f: &AutoFreezes) -> Vec<String> {
        f.freezes.iter().map(|x| x.module.clone()).collect()
    }
}
