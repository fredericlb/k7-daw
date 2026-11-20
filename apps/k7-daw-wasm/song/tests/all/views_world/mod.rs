// views_world/mod.rs — what the A2 tests share: a project read into `Inputs` from files in memory, a rig from JSON.
// A reader would get wrong the layout: a bare path is under the project (`/p/ws/proj`); `ws/`, `lib/` and `meta/` are taken as they stand.

#![allow(dead_code)]

use crate::support::{mem, mem_bare, mem_world_roots};
use indexmap::IndexMap;
use serde_json::Value;
use song::files::SettingsDoc;
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::modules::panels::Panels;
use song::modules::{LoadedModule, Scan, Vocabulary};
use song::resolve::Resolution;
use song::routing::Routing;
use song::{Cache, Entry, FamilyRead, Head, Kind, MemSource, Roots, Source, SourceError, Span, Stamp, Stat};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Where a file named by a test goes in the memory world.
pub fn placed(rel: &str) -> String {
    if ["ws/", "lib/", "meta/", "home/"].iter().any(|p| rel.starts_with(p)) {
        rel.to_owned()
    } else {
        format!("ws/proj/{rel}")
    }
}

pub fn world(files: &[(&str, &str)]) -> MemSource {
    let owned: Vec<(String, &str)> = files.iter().map(|(r, t)| (placed(r), *t)).collect();
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    mem(&refs)
}

/// The same with no root file added (811).
pub fn world_bare(files: &[(&str, &str)]) -> MemSource {
    let owned: Vec<(String, &str)> = files.iter().map(|(r, t)| (placed(r), *t)).collect();
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    mem_bare(&refs)
}

pub fn roots() -> Roots {
    mem_world_roots()
}

/// The inputs of a project laid out in memory, read with `rig`.
pub fn inputs_with(files: &[(&str, &str)], rig: &Value) -> Inputs {
    load_inputs(&world(files), &mut Cache::new(), &roots(), Rig::from_json(rig)).expect("inputs")
}

pub fn inputs(files: &[(&str, &str)]) -> Inputs {
    inputs_with(files, &serde_json::json!({}))
}

/// An `Inputs` built by hand: the project at `/p/ws/proj`, nothing read, every field empty but the vocabulary given.
pub fn bare(vocabulary: IndexMap<String, LoadedModule>) -> Inputs {
    Inputs {
        rig: Rig::default(),
        project: "/p/ws/proj".to_owned(),
        rule: song::PathRule::HOST,
        head: Head {
            root: None,
            markers: Vec::new(),
            settings: SettingsDoc::default(),
            findings: Vec::new(),
            compile_findings: Vec::new(),
            format_too_new: false,
        },
        routing: Routing::default(),
        nodes: song::modules::NodesRead::default(),
        vocabulary: Arc::new(Vocabulary {
            modules: vocabulary,
            folders: Vec::new(),
            findings: Vec::new(),
            scan: Scan::default(),
            panels: Panels::default(),
            broken: Vec::new(),
        }),
        local: Resolution::default(),
        extra: Resolution::default(),
        chains: IndexMap::new(),
        bypassed: HashSet::new(),
        bus_only: HashSet::new(),
        routed_taps: IndexMap::new(),
        clips: empty_family(),
        sequences: empty_family(),
        timelines: empty_family(),
        audio_paths: IndexMap::new(),
        warnings: Vec::new(),
    }
}

pub fn empty_family<T>() -> FamilyRead<T> {
    FamilyRead {
        values: Vec::new(),
        names: Vec::new(),
        findings: Vec::new(),
    }
}

/// A Source that answers at once: folders by name, files by name, nothing else; no scan of what it holds.
#[derive(Default)]
pub struct Fake {
    pub lists: HashMap<String, Vec<Entry>>,
    pub files: HashSet<String>,
}

impl Fake {
    pub fn dir(&mut self, path: &str, entries: Vec<Entry>) {
        self.lists.insert(path.to_owned(), entries);
    }

    pub fn file(&mut self, path: &str) {
        self.files.insert(path.to_owned());
    }
}

pub fn entry(name: &str, kind: Kind) -> Entry {
    Entry {
        name: name.to_owned(),
        kind,
    }
}

impl Source for Fake {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let stamp = Stamp {
            mtime_ns: 1,
            len: 1,
            settled: true,
        };
        if self.files.contains(path) {
            Ok(Stat {
                kind: Kind::File,
                len: 1,
                stamp,
            })
        } else if self.lists.contains_key(path) {
            Ok(Stat {
                kind: Kind::Dir,
                len: 0,
                stamp,
            })
        } else {
            Err(SourceError::NotFound)
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.lists.get(dir).cloned().ok_or(SourceError::NotFound)
    }

    fn read(&self, _: &str, _: Span) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::NotFound)
    }
}

/// One file of a family decoded by the cache, to share across a hundred thousand entries.
pub fn decoded<T: song::FileValue>(rel: &str, text: &str) -> Arc<T> {
    let src = world(&[(rel, text)]);
    let read = Cache::new()
        .read::<T>(&src, &format!("/p/ws/proj/{rel}"))
        .expect("reads");
    read.unwrap_or_else(|f| panic!("{rel}: {}", f.message))
}

/// A plugin module of a ref, for a vocabulary built by hand.
pub fn plugin_module(category: &str, name: &str, plugin: &str) -> LoadedModule {
    let mut m = LoadedModule::bare(song::modules::ModuleKind::Plugin, category, name);
    m.authored.plugin = Some(song::modules::Plugin {
        plugin_type: Some("internal".to_owned()),
        name: Some(plugin.to_owned()),
        ..song::modules::Plugin::default()
    });
    m
}
