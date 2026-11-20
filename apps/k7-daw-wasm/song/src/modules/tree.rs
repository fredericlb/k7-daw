// modules/tree.rs — one root's walk: type folders, one category level, module folders, engines, the nodes beside a module.
// A reader would get wrong that the depth is fixed by SHAPE (type, category, module, engines, engine): a listing is never followed deeper.

use super::assets::{Files, Places};
use super::decorate::stamp_files;
use super::merge::merge_authored;
use super::types::{
    DSP_FILE, DSP_TYPE, ENGINES_DIR, JSDSP_FILE, JSDSP_TYPE, MODULE_FILE, MODULE_OWN_BASES, MODULE_TYPES, TypeMeta,
    engine_ref_of, is_name, module_key, ports,
};
use super::weight::module_entries;
use super::{LoadedModule, Module};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::read::doc_files;
use crate::roots::ModuleRoot;
use crate::source::path::{join, windows_hazard};
use crate::source::{Entry, Kind, Source, SourceError};
use indexmap::IndexMap;
use std::sync::Arc;

/// A module folder the walk went through: the ref it answers to and where it is, what 2c looks `ui.k7x` up in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleFolder {
    pub module_ref: String,
    pub path: String,
    /// The library's name, the namespace of its refs.
    pub label: String,
    pub engine: bool,
}

/// What one root's walk produced.
#[derive(Debug, Default)]
pub struct TreeRead {
    pub modules: IndexMap<String, LoadedModule>,
    pub folders: Vec<ModuleFolder>,
    /// Warnings: a module file that did not read, an entry the Source refused.
    pub findings: Vec<Finding>,
    /// The files that did not decode, as named in the findings: what an `extends` of an absent module may say.
    pub broken: Vec<String>,
    /// Whether the walk stopped at its budget.
    pub capped: bool,
}

struct Walk<'a> {
    source: &'a dyn Source,
    cache: &'a mut Cache,
    namespace: &'a str,
    root: &'a str,
    places: &'a Places,
    budget: usize,
    /// What the walk has produced: modules, folders, findings, engine refs copied and map entries merged.
    weight: usize,
    out: TreeRead,
}

/// A fixed name of the tree is a file when it is probed; a folder or nothing is not.
enum Probe {
    File,
    Absent,
}

/// The decoded authored fields, or the finding that says the file did not read.
enum Loaded {
    Module(Arc<Module>),
    Broken,
    Gone,
}

fn warning(file: String, message: String) -> Finding {
    Finding {
        file,
        line: 1,
        col: 1,
        message,
        severity: Severity::Warning,
    }
}

impl Walk<'_> {
    fn full(&self) -> bool {
        self.out.capped || self.weight >= self.budget
    }

    fn charge(&mut self, n: usize) {
        self.weight = self.weight.saturating_add(n);
    }

    /// A finding, which weighs one: a folder of broken files must not grow the output without a stop.
    fn find(&mut self, f: Finding) {
        self.charge(1);
        self.out.findings.push(f);
    }

    fn stop(&mut self) {
        if !self.out.capped {
            self.out.capped = true;
            self.out.findings.push(warning(
                format!("@{}", self.namespace),
                format!(
                    "the module tree holds more than {} entries — the walk stopped",
                    self.budget
                ),
            ));
        }
    }

    /// What the walk calls a path in a finding: the ref it would have.
    fn label(&self, path: &str) -> String {
        let rel = path.strip_prefix(self.root).unwrap_or(path).trim_start_matches('/');
        format!("@{}/{rel}", self.namespace)
    }

    fn refused(&mut self, path: &str) {
        let file = self.label(path);
        self.find(warning(
            file,
            "outside the folders this compile may read — skipped".to_owned(),
        ));
    }

    /// `name` under `dir`, or None when Windows would read it as more than a name.
    fn child(&mut self, dir: &str, name: &str) -> Option<String> {
        let path = join(dir, name);
        if self.places.rule.is_windows() && windows_hazard(&path) {
            self.refused(&path);
            return None;
        }
        Some(path)
    }

    /// The entries of a folder; one that is absent or cannot be listed is empty, one the Source refuses is warned.
    fn entries(&mut self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        match self.source.list(dir) {
            Ok(e) => Ok(e),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Vec::new()),
            Err(SourceError::Outside) => {
                self.refused(dir);
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }

    /// The sub-folders of a folder, dot entries skipped, in the listing's order.
    fn dirs(&mut self, dir: &str) -> Result<Vec<(String, String)>, SourceError> {
        let mut out = Vec::new();
        for e in self.entries(dir)? {
            if e.kind == Kind::Dir
                && !e.name.starts_with('.')
                && let Some(path) = self.child(dir, &e.name)
            {
                out.push((e.name, path));
            }
        }
        Ok(out)
    }

    fn is_dir(&mut self, path: &str) -> Result<bool, SourceError> {
        match self.source.stat(path) {
            Ok(st) => Ok(st.kind == Kind::Dir),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(false),
            Err(SourceError::Outside) => {
                self.refused(path);
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Whether `folder` holds `module.k7x`, probed by `stat` so a case-insensitive file system finds `Module.K7X`.
    fn module_file(&mut self, folder: &str) -> Result<Probe, SourceError> {
        let path = join(folder, &format!("{MODULE_FILE}.k7x"));
        match self.source.stat(&path) {
            Ok(st) if st.kind == Kind::File => Ok(Probe::File),
            Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Probe::Absent),
            Err(SourceError::Outside) => {
                self.refused(&path);
                Ok(Probe::Absent)
            }
            Err(e) => Err(e),
        }
    }

    fn load(&mut self, path: &str, named: &str) -> Result<Loaded, SourceError> {
        match self.cache.read::<Module>(self.source, path) {
            Ok(Ok(m)) => Ok(Loaded::Module(m)),
            Ok(Err(f)) => {
                // Named by its REF, not by a path: the file is outside the project, and the ref is what a chain says.
                self.out.broken.push(named.to_owned());
                self.find(Finding {
                    file: named.to_owned(),
                    message: format!("did not load — {}", f.message),
                    severity: Severity::Warning,
                    ..f
                });
                Ok(Loaded::Broken)
            }
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Loaded::Gone),
            Err(SourceError::Outside) => {
                self.refused(path);
                Ok(Loaded::Gone)
            }
            Err(e) => Err(e),
        }
    }

    /// The engine names a module folder holds: each sub-folder of `engines/` with a module file in it.
    fn engine_folders(&mut self, folder: &str) -> Result<Vec<String>, SourceError> {
        let dir = join(folder, ENGINES_DIR);
        let mut names = Vec::new();
        for e in self.entries(&dir)? {
            if self.full() {
                self.stop();
                break;
            }
            if e.kind != Kind::Dir || !is_name(&e.name) {
                continue;
            }
            let Some(path) = self.child(&dir, &e.name) else {
                continue;
            };
            if matches!(self.module_file(&path)?, Probe::File) {
                self.charge(1);
                names.push(e.name);
            }
        }
        names.sort();
        Ok(names)
    }

    fn note_folder(&mut self, module_ref: &str, path: &str, engine: bool) {
        self.charge(1);
        self.out.folders.push(ModuleFolder {
            module_ref: module_ref.to_owned(),
            path: path.to_owned(),
            label: self.namespace.to_owned(),
            engine,
        });
    }

    /// A module's own ports when it declares some, else its type folder's.
    fn decorate(
        &mut self,
        authored: &Module,
        meta: &TypeMeta,
        folder: &str,
        node: Option<&str>,
        named: &str,
    ) -> Result<LoadedModule, SourceError> {
        let category = meta.name;
        let mut m = LoadedModule::bare(meta.kind, category, folder.rsplit('/').next().unwrap_or(folder));
        m.authored = authored.clone();
        let own = |l: &Option<Vec<super::Port>>| l.clone().filter(|v| !v.is_empty());
        m.authored.inputs = Some(own(&authored.inputs).unwrap_or_else(|| ports(meta.inputs)));
        m.authored.outputs = Some(own(&authored.outputs).unwrap_or_else(|| ports(meta.outputs)));
        match authored.plugin.as_ref().and_then(|p| p.plugin_type.as_deref()) {
            Some(JSDSP_TYPE) => m.jsdsp = Some(join(folder, JSDSP_FILE)),
            Some(DSP_TYPE) => m.jsdsp = Some(join(folder, DSP_FILE)),
            _ => {}
        }
        m.folder_abs = Some(folder.to_owned());
        let mut files = Files::new(self.source, self.places, named);
        stamp_files(
            &mut files,
            &mut m,
            folder,
            folder.rsplit('/').next().unwrap_or(folder),
            node,
        )?;
        for f in files.findings {
            self.find(f);
        }
        Ok(m)
    }

    /// One module folder: its engines first, then the module, then the nodes beside it.
    fn read_module(&mut self, meta: &TypeMeta, folder: &str, sub: &str, is_engine: bool) -> Result<(), SourceError> {
        if self.full() {
            self.stop();
            return Ok(());
        }
        let key = format!("@{}/{}/{sub}", self.namespace, meta.name);
        let file = join(folder, &format!("{MODULE_FILE}.k7x"));
        let named = format!("{key}/{MODULE_FILE}.k7x");
        let Loaded::Module(m) = self.load(&file, &named)? else {
            return Ok(());
        };
        let engines = if is_engine {
            Vec::new()
        } else {
            self.engine_folders(folder)?
        };
        let engine_refs: IndexMap<String, String> =
            engines.iter().map(|e| (e.clone(), engine_ref_of(&key, e))).collect();
        for e in &engines {
            let Some(path) = self.child(&join(folder, ENGINES_DIR), e) else {
                continue;
            };
            self.note_folder(&engine_ref_of(&key, e), &path, true);
            self.read_module(meta, &path, &format!("{sub}/{ENGINES_DIR}/{e}"), true)?;
            if let Some(loaded) = self.out.modules.get_mut(&engine_ref_of(&key, e)) {
                loaded.engine_of = Some(key.clone());
            }
        }
        let with_engines = |mut lm: LoadedModule| {
            if !engine_refs.is_empty() {
                lm.engine_refs = Some(engine_refs.clone());
            }
            lm
        };
        self.charge(1 + engine_refs.len() + module_entries(&m));
        let decorated = self.decorate(&m, meta, folder, None, &named)?;
        self.out.modules.insert(key.clone(), with_engines(decorated));
        self.read_nodes(meta, folder, &key, &m, engine_refs.len(), &with_engines)
    }

    /// The nodes beside `module.k7x`, keyed like a local one under the ref; without them an inherited node is unnameable.
    fn read_nodes(
        &mut self,
        meta: &TypeMeta,
        folder: &str,
        key: &str,
        m: &Module,
        refs: usize,
        with_engines: &dyn Fn(LoadedModule) -> LoadedModule,
    ) -> Result<(), SourceError> {
        let listing = self.entries(folder)?;
        let mut nodes = doc_files(&listing);
        nodes.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        for (node, name) in nodes {
            if MODULE_OWN_BASES.contains(&node.as_str()) || node.starts_with('.') {
                continue;
            }
            if self.full() {
                self.stop();
                break;
            }
            let Some(path) = self.child(folder, &name) else {
                continue;
            };
            let Loaded::Module(value) = self.load(&path, &format!("{key}/{name}"))? else {
                continue;
            };
            let mut merged = merge_authored(m, &value);
            // A grid is taken whole, never half-merged.
            merged.ui = value.ui.clone().or_else(|| m.ui.clone());
            // Each node carries a copy of its module's merged maps and engine refs, which is what it weighs.
            self.charge(1 + refs + module_entries(&merged));
            let mut lm = self.decorate(&merged, meta, folder, Some(&node), &format!("{key}/{name}"))?;
            lm.node = Some(node.clone());
            self.out.modules.insert(module_key(key, Some(&node)), with_engines(lm));
        }
        Ok(())
    }

    fn walk_type(&mut self, meta: &TypeMeta) -> Result<(), SourceError> {
        let abs = join(self.root, meta.name);
        if !self.is_dir(&abs)? {
            return Ok(());
        }
        for (name, folder) in self.dirs(&abs)? {
            if self.full() {
                self.stop();
                return Ok(());
            }
            let r = format!("@{}/{}/{name}", self.namespace, meta.name);
            self.note_folder(&r, &folder, false);
            // A folder with no module file is a CATEGORY: one level deep only, a filing convenience.
            if matches!(self.module_file(&folder)?, Probe::File) {
                self.read_module(meta, &folder, &name, false)?;
                continue;
            }
            for (leaf, leaf_folder) in self.dirs(&folder)? {
                if self.full() {
                    self.stop();
                    return Ok(());
                }
                self.note_folder(&format!("{r}/{leaf}"), &leaf_folder, false);
                if matches!(self.module_file(&leaf_folder)?, Probe::File) {
                    self.read_module(meta, &leaf_folder, &format!("{name}/{leaf}"), false)?;
                }
            }
        }
        Ok(())
    }
}

/// Walks one root, its output weighed against `budget` (modules, folders, findings, engine refs, merged entries): past it one warning.
pub fn read_tree(
    source: &dyn Source,
    cache: &mut Cache,
    root: &ModuleRoot,
    budget: usize,
) -> Result<TreeRead, SourceError> {
    read_tree_in(source, cache, root, budget, &Places::default())
}

/// The same walk, a module's files resolved against `places` (no project: an `@project/` ref stays for each compile).
pub fn read_tree_in(
    source: &dyn Source,
    cache: &mut Cache,
    root: &ModuleRoot,
    budget: usize,
    places: &Places,
) -> Result<TreeRead, SourceError> {
    let mut walk = Walk {
        source,
        cache,
        namespace: &root.namespace,
        root: &root.dir,
        places,
        budget,
        weight: 0,
        out: TreeRead::default(),
    };
    for meta in &MODULE_TYPES {
        walk.walk_type(meta)?;
        if walk.out.capped {
            break;
        }
    }
    Ok(walk.out)
}
