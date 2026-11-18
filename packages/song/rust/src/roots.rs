// roots.rs — the places a project reads from, as the HOST resolved them: the crate reads under them and never finds them.
// A reader would get wrong that a namespace comes from a root's PLACE here (workspace, a library's name, meta), never from a path.

use crate::modules::types::{META_NS, SHARED_MODULES_DIR};
use crate::source::SourceError;
use crate::source::path::{PathRule, is_within, join, normalize};
use serde::{Deserialize, Serialize};

/// One library: its name (the namespace of its refs), the folder it is read from, and whether this account writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub name: String,
    pub root: String,
    pub editable: bool,
}

/// Where a compile reads: the project, the workspace beside it, the libraries nearest first, k7's own modules, the scan.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roots {
    pub project: String,
    /// The project's PARENT folder: its `_modules/` is the `@workspace` level, as the TypeScript's `../_modules`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The level of the project's `_modules/` (absent means `workspace`): a project inside a library sees it as that library's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_namespace: Option<String>,
    /// The whole workspace the project stands in (the TypeScript's `workspaceDirOf`), mounted so a file it names is readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confine: Option<String>,
    /// Account first, then by name: the host's precedence order.
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// k7's own modules, read like a library under `@meta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
    /// The engine's scan, a FILE (`exts.json`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<String>,
    /// The automatic freezes' renders: declared and unread until the sub-step that stats them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renders: Option<String>,
    /// The models cache: a folder mount that is only ever stat'ed, never read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<String>,
    #[serde(default)]
    pub sample_banks: Vec<String>,
    /// Folders mounted and read with no namespace: the other versions of a library, and a linked family folder's real path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_readable: Vec<String>,
    /// The path rule of the machine that spelled these roots, which a replay elsewhere reads them by.
    #[serde(default)]
    pub paths: PathRule,
}

/// A folder holding type folders, with the namespace its refs carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleRoot {
    pub namespace: String,
    pub dir: String,
}

/// A path in the Source's spelling; under the Windows rule `\` separates and a drive letter is upper-cased.
pub fn spell(path: &str, rule: PathRule) -> Result<String, SourceError> {
    if rule.is_windows() {
        normalize(&path.replace('\\', "/"))
    } else {
        normalize(path)
    }
}

/// Whether a ref is absolute under the rule; a drive letter is a name under the Posix one.
pub fn is_absolute(r: &str, rule: PathRule) -> bool {
    if rule.is_windows() {
        spell(r, rule).is_ok()
    } else {
        r.starts_with('/')
    }
}

impl Roots {
    /// The module roots a project sees, nearest first: the workspace's `_modules/`, each library's `modules/`, then `@meta`.
    pub fn module_roots(&self) -> Result<Vec<ModuleRoot>, SourceError> {
        let rule = self.paths;
        let mut out: Vec<ModuleRoot> = Vec::new();
        let mut add = |namespace: &str, dir: String| {
            // A root listed twice is read once, under the first namespace that named it.
            if !out.iter().any(|r| r.dir == dir) {
                out.push(ModuleRoot {
                    namespace: namespace.to_owned(),
                    dir,
                });
            }
        };
        if let Some(ws) = &self.workspace {
            let ns = self.workspace_namespace.as_deref().unwrap_or("workspace");
            add(ns, join(&spell(ws, rule)?, SHARED_MODULES_DIR));
        }
        for lib in &self.libraries {
            add(&lib.name, join(&spell(&lib.root, rule)?, "modules"));
        }
        if let Some(meta) = &self.meta {
            add(META_NS.trim_start_matches('@'), spell(meta, rule)?);
        }
        Ok(out)
    }

    /// What a Source must serve to read all of this: the folder mounts, and the scan as a file mount.
    /// The whole workspace is `confine`, a root of its own; `renders` and `models` are mounted to be stat'ed.
    pub fn mounts(&self) -> Result<(Vec<String>, Vec<String>), SourceError> {
        let rule = self.paths;
        let mut dirs = vec![spell(&self.project, rule)?];
        let mut others: Vec<String> = Vec::new();
        if let Some(ws) = &self.workspace {
            others.push(join(&spell(ws, rule)?, SHARED_MODULES_DIR));
        }
        let extra = [&self.confine, &self.renders, &self.models];
        for d in extra.into_iter().flatten() {
            others.push(spell(d, rule)?);
        }
        for d in self.meta.iter().chain(self.libraries.iter().map(|l| &l.root)) {
            others.push(spell(d, rule)?);
        }
        for d in &self.also_readable {
            others.push(spell(d, rule)?);
        }
        for d in others {
            if !dirs.iter().any(|x| is_within(x, &d)) {
                dirs.push(d);
            }
        }
        let files = match &self.scan {
            Some(f) => vec![spell(f, rule)?],
            None => Vec::new(),
        };
        Ok((dirs, files))
    }

    /// The folder of the library `name`, as the host gave it, in the Source's spelling.
    pub fn library_root(&self, name: &str) -> Result<Option<String>, SourceError> {
        let rule = self.paths;
        self.libraries
            .iter()
            .find(|l| l.name == name)
            .map(|l| spell(&l.root, rule))
            .transpose()
    }

    /// What the vocabulary is keyed by: the module roots with their namespaces, the libraries, the models root, the scan file.
    pub fn vocabulary_key(&self) -> String {
        let rule = self.paths;
        let roots: Vec<(String, String)> = self
            .module_roots()
            .map(|r| r.into_iter().map(|m| (m.namespace, m.dir)).collect())
            .unwrap_or_default();
        let scan = self.scan.as_deref().map(|d| spell(d, rule)).and_then(Result::ok);
        let models = self.models.as_deref().map(|d| spell(d, rule)).and_then(Result::ok);
        let libraries: Vec<(&str, Option<String>)> = self
            .libraries
            .iter()
            .map(|l| (l.name.as_str(), spell(&l.root, rule).ok()))
            .collect();
        let also: Vec<String> = self.also_readable.iter().filter_map(|d| spell(d, rule).ok()).collect();
        serde_json::to_string(&(roots, scan, models, libraries, also, self.paths)).unwrap_or_default()
    }
}
