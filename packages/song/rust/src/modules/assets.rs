// modules/assets.rs — the files a module LOADS, resolved where it declares them: rack audio, file params, a library's files.
// A reader would get wrong that the Source confines: a path it refuses is no file and a warning, whatever the TypeScript read.

use super::files::{FILE_PARAMS, LIBRARY_FILE_PARAMS, PROJECT_SOUNDS, library_family_of, shared_family_of};
use super::types::is_library_name;
use super::{Alternative, LoadedModule, Module, ParamValue, Rack};
use crate::files::Finding;
use crate::files::Severity;
use crate::files::scalar::NonEmpty;
use crate::roots::{Roots, is_absolute, spell};
use crate::source::path::{PathRule, join, join_within, windows_hazard};
use crate::source::{Entry, Kind, Source, SourceError};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Mutex;

/// The two folders a module's own files are looked for in, below it.
pub const MODULE_ASSETS_DIR: &str = "assets";
pub const MODULE_SOURCES_DIR: &str = "sources";

/// Where a compile looks for a file a ref names, as the host resolved them: the project (per compile), libraries, models.
#[derive(Debug, Default)]
pub struct Places {
    /// None while the vocabulary is built: an `@project/` ref is resolved per compile, outside it.
    pub project: Option<String>,
    pub libraries: Vec<(String, String)>,
    pub models: Option<String>,
    /// The path rule of the roots these places come from.
    pub rule: PathRule,
    /// The listings of library roots seen in this call: whether a family folder is there is read off its root's listing.
    listed: Mutex<HashMap<String, Vec<Entry>>>,
}

impl Places {
    pub fn of(roots: &Roots, with_project: bool) -> Result<Places, SourceError> {
        let rule = roots.paths;
        let libraries = roots
            .libraries
            .iter()
            .map(|l| Ok((l.name.clone(), spell(&l.root, rule)?)))
            .collect::<Result<Vec<_>, SourceError>>()?;
        Ok(Places {
            project: if with_project {
                Some(spell(&roots.project, rule)?)
            } else {
                None
            },
            libraries,
            models: roots.models.as_deref().map(|d| spell(d, rule)).transpose()?,
            rule,
            listed: Mutex::default(),
        })
    }
}

/// What a stat said, with a path the Source refuses counted as absent.
enum Seen {
    File,
    Dir,
    Absent,
}

/// Resolves refs to paths through a Source, naming a file the Source refuses in a warning at `at`.
pub struct Files<'a> {
    source: &'a dyn Source,
    places: &'a Places,
    at: String,
    pub findings: Vec<Finding>,
}

fn text_of<'p>(params: &'p Option<IndexMap<String, ParamValue>>, key: &str) -> Option<&'p str> {
    match params.as_ref()?.get(key)? {
        ParamValue::Text(t) => Some(t),
        ParamValue::Number(_) => None,
    }
}

impl<'a> Files<'a> {
    pub fn new(source: &'a dyn Source, places: &'a Places, at: &str) -> Files<'a> {
        Files {
            source,
            places,
            at: at.to_owned(),
            findings: Vec::new(),
        }
    }

    pub fn places(&self) -> &Places {
        self.places
    }

    pub fn source(&self) -> &dyn Source {
        self.source
    }

    fn refuse(&mut self, path: &str) {
        let message = format!("{path}: outside the folders this compile may read — ignored");
        if !self.findings.iter().any(|f| f.message == message) {
            self.findings.push(Finding {
                file: self.at.clone(),
                line: 1,
                col: 1,
                message,
                severity: Severity::Warning,
            });
        }
    }

    fn probe(&mut self, path: &str) -> Result<Seen, SourceError> {
        match self.source.stat(path) {
            Ok(st) if st.kind == Kind::File => Ok(Seen::File),
            Ok(_) => Ok(Seen::Dir),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Seen::Absent),
            Err(SourceError::Outside) => {
                self.refuse(path);
                Ok(Seen::Absent)
            }
            Err(e) => Err(e),
        }
    }

    /// Whether anything is at `path`.
    pub fn exists(&mut self, path: &str) -> Result<bool, SourceError> {
        Ok(!matches!(self.probe(path)?, Seen::Absent))
    }

    /// Whether a FILE is at `path`; a folder answers as absent.
    pub fn is_file(&mut self, path: &str) -> Result<bool, SourceError> {
        Ok(matches!(self.probe(path)?, Seen::File))
    }

    /// `path` when the Source admits it (present or not), else nothing.
    fn confined(&mut self, path: String) -> Result<Option<String>, SourceError> {
        match self.source.stat(&path) {
            Ok(_) | Err(SourceError::NotFound) => Ok(Some(path)),
            Err(SourceError::Outside) => {
                self.refuse(&path);
                Ok(None)
            }
            Err(SourceError::Io(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `<folder>/<sub>/<ref>` for the first folder holding it; a ref escaping `<folder>/<sub>` ends the search.
    pub fn module_file_in(
        &mut self,
        r: &str,
        folders: &[Option<&str>],
        sub: &str,
    ) -> Result<Option<String>, SourceError> {
        if r.is_empty() || is_absolute(r, self.places.rule) || r.contains('\0') {
            return Ok(None);
        }
        for folder in folders.iter().flatten() {
            let root = join(folder, sub);
            let Some(abs) = join_within(&root, r) else {
                return Ok(None);
            };
            if self.exists(&abs)? {
                return Ok(Some(abs));
            }
        }
        Ok(None)
    }

    /// A module's own `assets/<ref>`, with the machine's model cache as the one fallback; the cache is only stat'ed.
    pub fn module_asset_in(&mut self, r: &str, folders: &[Option<&str>]) -> Result<Option<String>, SourceError> {
        if let Some(own) = self.module_file_in(r, folders, MODULE_ASSETS_DIR)? {
            return Ok(Some(own));
        }
        if r.is_empty() || is_absolute(r, self.places.rule) {
            return Ok(None);
        }
        let Some(root) = self.places.models.clone() else {
            return Ok(None);
        };
        match join_within(&root, r) {
            Some(cached) if self.exists(&cached)? => Ok(Some(cached)),
            _ => Ok(None),
        }
    }

    /// Whether a library's root lists `family` as a folder; a link is listed as what it points at, and is never confined.
    fn has_family(&mut self, root: &str, family: &str) -> Result<bool, SourceError> {
        let known = self.places.listed.lock().ok().and_then(|m| m.get(root).cloned());
        let entries = match known {
            Some(e) => e,
            None => {
                let e = match self.source.list(root) {
                    Ok(e) => e,
                    Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => Vec::new(),
                    Err(e) => return Err(e),
                };
                if let Ok(mut m) = self.places.listed.lock() {
                    m.insert(root.to_owned(), e.clone());
                }
                e
            }
        };
        Ok(entries.iter().any(|e| e.name == family && e.kind == Kind::Dir))
    }

    /// A library's family folder, when `name` is a library the host listed and the folder is there.
    fn library_family_root(&mut self, name: &str, family: &str) -> Result<Option<String>, SourceError> {
        if !(is_library_name(name) || name == "account") {
            return Ok(None);
        }
        let Some((_, root)) = self.places.libraries.iter().find(|(n, _)| n == name) else {
            return Ok(None);
        };
        Ok(self.has_family(root, family)?.then(|| join(root, family)))
    }

    /// The file a `@<library>/<path>` or `@project/<path>` ref names in `family`, never outside it.
    pub fn library_file(&mut self, family: &str, r: &str) -> Result<Option<String>, SourceError> {
        let Some((lib, tail)) = r.strip_prefix('@').and_then(|rest| rest.split_once('/')) else {
            return Ok(None);
        };
        let hazard = self.places.rule.is_windows() && windows_hazard(tail);
        if lib.is_empty() || tail.is_empty() || hazard || tail.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
            return Ok(None);
        }
        let root = if lib == PROJECT_SOUNDS {
            self.places.project.as_ref().map(|p| join(p, family))
        } else {
            self.library_family_root(lib, family)?
        };
        let Some(root) = root else { return Ok(None) };
        // Named, never opened by the compile, and returned unchecked as the TypeScript does: the engine reads it.
        Ok(join_within(&root, tail).filter(|abs| *abs != root))
    }

    /// A file param's file: a library's when the param may name one and the ref says `@`, else the module's own `assets/`.
    pub fn file_param_in(
        &mut self,
        param: &str,
        r: &str,
        folders: &[Option<&str>],
    ) -> Result<Option<String>, SourceError> {
        if let Some(family) = shared_family_of(param)
            && r.starts_with('@')
        {
            return self.library_file(family, r);
        }
        self.module_asset_in(r, folders)
    }

    /// A rack's audio ref: `sources/`, `assets/`, then joined to the folder, and confined by the Source.
    fn at_ref(&mut self, f: &str, folder: &str) -> Result<Option<String>, SourceError> {
        if f.contains('\0') {
            return Ok(None);
        }
        let found = if is_absolute(f, self.places.rule) {
            spell(f, self.places.rule).ok()
        } else if let Some(p) = self.module_file_in(f, &[Some(folder)], MODULE_SOURCES_DIR)? {
            Some(p)
        } else if let Some(p) = self.module_asset_in(f, &[Some(folder)])? {
            Some(p)
        } else {
            crate::source::path::normalize(&join(folder, f)).ok()
        };
        match found {
            Some(p) => self.confined(p),
            None => Ok(None),
        }
    }

    /// A rack's audio refs rewritten to absolute paths in the declaring folder, so racks merge without provenance.
    pub fn rack_audio(&mut self, rack: &mut Rack, folder: &str) -> Result<(), SourceError> {
        if let Some(src) = rack.source.take() {
            rack.source = if src.is_empty() {
                Some(src)
            } else {
                self.at_ref(&src, folder)?
            };
        }
        for sound in rack.symbols.values_mut() {
            let found = match (&sound.sample, &sound.clip) {
                (Some(sample), _) => self.library_file("samples", &sample.0)?,
                (None, Some(clip)) => self.at_ref(&clip.0, folder)?,
                (None, None) => None,
            };
            sound.clip = found.map(NonEmpty);
        }
        Ok(())
    }

    /// Each alternative's own rack, resolved as the module's is: the set is merged over the module later.
    pub fn alternative_racks(&mut self, m: &mut Module, folder: &str) -> Result<(), SourceError> {
        for alt in m.alternatives.iter_mut().flatten() {
            if let Some(rack) = alt.rack.as_mut() {
                self.rack_audio(rack, folder)?;
            }
        }
        Ok(())
    }

    /// The file-valued params a module names, resolved inside its own folder, and the names as authored.
    pub fn resolve_assets(&mut self, lm: &mut LoadedModule, folder: &str) -> Result<(), SourceError> {
        let get = |f: &str| text_of(&lm.authored.params, f).map(str::to_owned);
        let files: Vec<(&str, Option<String>)> = FILE_PARAMS.iter().map(|f| (*f, get(f))).collect();
        let shared: Vec<(&str, &str, Option<String>)> =
            LIBRARY_FILE_PARAMS.iter().map(|(f, fam)| (*f, *fam, get(f))).collect();
        let mut assets: IndexMap<String, String> = IndexMap::new();
        for (field, r) in files {
            let found = match &r {
                Some(r) => self.file_param_in(field, r, &[Some(folder)])?,
                None => None,
            };
            *file_slot(lm, field) = found;
            if let Some(r) = r.filter(|r| !r.is_empty()) {
                assets.insert(field.to_owned(), r);
            }
        }
        for (field, family, r) in shared {
            if let Some(r) = r.filter(|r| !r.is_empty()) {
                *file_slot(lm, field) = self.library_file(family, &r)?;
                assets.insert(field.to_owned(), r);
            }
        }
        lm.assets = (!assets.is_empty()).then_some(assets);
        Ok(())
    }

    /// Resolves what a module authored as `@project/…`, against the project of this compile; a field already set stays.
    pub fn project_pass(&mut self, lm: &mut LoadedModule) -> Result<(), SourceError> {
        if self.places.project.is_none() {
            return Ok(());
        }
        let named: Vec<(String, String)> = lm
            .assets
            .iter()
            .flatten()
            .filter(|(_, v)| v.starts_with("@project/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (param, r) in named {
            let family = shared_family_of(&param).or_else(|| library_family_of(&param));
            let unset = file_slot(lm, &param).is_none();
            if let (Some(family), true) = (family, unset) {
                let found = self.library_file(family, &r)?;
                *file_slot(lm, &param) = found;
            }
        }
        if let Some(rack) = lm.authored.rack.as_mut() {
            self.project_samples(rack)?;
        }
        for alt in lm.authored.alternatives.iter_mut().flatten() {
            if let Some(rack) = alt.rack.as_mut() {
                self.project_samples(rack)?;
            }
        }
        Ok(())
    }

    fn project_samples(&mut self, rack: &mut Rack) -> Result<(), SourceError> {
        for sound in rack.symbols.values_mut() {
            if sound.clip.is_none()
                && let Some(sample) = sound.sample.as_ref().filter(|s| s.0.starts_with("@project/"))
            {
                sound.clip = self.library_file("samples", &sample.0)?.map(NonEmpty);
            }
        }
        Ok(())
    }
}

/// The field a file param lands in.
pub fn file_slot<'m>(lm: &'m mut LoadedModule, param: &str) -> &'m mut Option<String> {
    match param {
        "model" => &mut lm.model,
        "onnx" => &mut lm.onnx,
        _ => &mut lm.ir,
    }
}

/// Whether anything of a module names an `@project/` file, which only a compile's project can resolve.
pub fn mentions_project(lm: &LoadedModule) -> bool {
    let rack = |r: &Option<Rack>| {
        r.iter()
            .flat_map(|r| r.symbols.values())
            .any(|s| s.sample.as_ref().is_some_and(|x| x.0.starts_with("@project/")))
    };
    lm.assets.iter().flatten().any(|(_, v)| v.starts_with("@project/"))
        || rack(&lm.authored.rack)
        || lm
            .authored
            .alternatives
            .iter()
            .flatten()
            .any(|a: &Alternative| rack(&a.rack))
}
