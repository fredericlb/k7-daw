// graph/view/choices.rs — what a node's fields may be SET to: its assets, bases, shaders and the libraries' files, discovered by walking folders.
// A reader would get wrong that every walk is the Source's and charged: a link is a folder or a file by what it points at, never a loop (422).

use super::amp::{AmpModel, amp_model_info};
use super::web::WebChainStep;
use crate::files::{Finding, Severity};
use crate::graph::{GraphModule, MAX_ANCESTRY, Spend};
use crate::lanes::inputs::Inputs;
use crate::modules::files::{LIBRARY_FILE_PARAMS, PROJECT_SOUNDS, is_library_file_of, shared_family_of};
use crate::modules::weight::weigh;
use crate::resolve::{ResolvedUi, resolve_ref};
use crate::roots::Roots;
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// How deep each family of walks goes below its folder.
const ASSETS_DEPTH: usize = 6;
const BASES_DEPTH: usize = 3;
const LIBRARY_DEPTH: usize = 6;
/// The extensions an asset folder is walked for: every one a file-valued param takes.
const ASSET_EXTS: [&str; 3] = [".nam", ".json", ".onnx"];
const BASE_EXTS: [&str; 2] = [".vstpreset", ".fxp"];

/// A walk's answer: none when the budget stopped it.
pub(super) type Walk<T> = Result<Option<T>, SourceError>;

/// The listing of one folder, as `assets_in`, `bases_in` and `shaders_in` answer it.
type Lister<'a> = fn(&mut Walks<'a>, &str, &mut Spend, &str) -> Walk<Arc<Vec<String>>>;

/// One ancestor as a walk climbs: where its files are and what it extends.
struct Parent {
    folder: Option<String>,
    extends: Option<String>,
}

/// The folders a graph's choices are walked in, each listed once for the whole call (424).
pub(super) struct Walks<'a> {
    source: &'a dyn Source,
    roots: &'a Roots,
    inputs: &'a Inputs,
    assets: HashMap<String, Arc<Vec<String>>>,
    bases: HashMap<String, Arc<Vec<String>>>,
    shaders: HashMap<String, Arc<Vec<String>>>,
    library: HashMap<(String, bool), Arc<Vec<String>>>,
    families: HashMap<String, Option<HashSet<String>>>,
    parents: HashMap<String, Option<Parent>>,
    warned: HashSet<String>,
    pub amp: HashMap<String, Option<AmpModel>>,
    pub findings: Vec<Finding>,
}

/// Strings in the order JavaScript's default `sort()` puts them: by UTF-16 code unit.
fn js_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn is_dot(name: &str) -> bool {
    name.starts_with('.')
}

impl<'a> Walks<'a> {
    pub fn new(source: &'a dyn Source, roots: &'a Roots, inputs: &'a Inputs) -> Walks<'a> {
        Walks {
            source,
            roots,
            inputs,
            assets: HashMap::new(),
            bases: HashMap::new(),
            shaders: HashMap::new(),
            library: HashMap::new(),
            families: HashMap::new(),
            parents: HashMap::new(),
            warned: HashSet::new(),
            amp: HashMap::new(),
            findings: Vec::new(),
        }
    }

    fn warn_outside(&mut self, folder: &str, file: &str) {
        if self.warned.insert(folder.to_owned()) {
            self.findings.push(Finding {
                file: file.to_owned(),
                line: 1,
                col: 1,
                message: "a folder this module lists lies outside the project's folders — its files are not offered"
                    .to_owned(),
                severity: Severity::Warning,
            });
        }
    }

    /// A listing, empty when the folder is absent or unreadable, empty with one warning when refused (423).
    fn entries(&mut self, dir: &str, file: &str) -> Result<Vec<crate::source::Entry>, SourceError> {
        match self.source.list(dir) {
            Ok(e) => Ok(e),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Vec::new()),
            Err(SourceError::Outside) => {
                self.warn_outside(dir, file);
                Ok(Vec::new())
            }
            Err(e) => Err(e),
        }
    }

    /// Every file under `root` as `root`-relative paths, depth first in the Source's order, dot names skipped, each entry one unit (422).
    pub fn list_under(&mut self, root: &str, max_depth: usize, spend: &mut Spend, file: &str) -> Walk<Vec<String>> {
        let mut out = Vec::new();
        let first = self.entries(root, file)?;
        // Each frame: the entries left, the relative prefix and the depth below `root`.
        let mut stack = vec![(first.into_iter().rev().collect::<Vec<_>>(), String::new(), 0usize)];
        while let Some((left, prefix, depth)) = stack.last_mut() {
            let Some(e) = left.pop() else {
                stack.pop();
                continue;
            };
            if is_dot(&e.name) {
                continue;
            }
            if !spend.charge(1) {
                return Ok(None);
            }
            let rel = if prefix.is_empty() {
                e.name.clone()
            } else {
                format!("{prefix}/{}", e.name)
            };
            match e.kind {
                Kind::File => out.push(rel),
                Kind::Dir if *depth < max_depth => {
                    let below = *depth + 1;
                    let dir = join(root, &rel);
                    let inner = self.entries(&dir, file)?;
                    stack.push((inner.into_iter().rev().collect(), rel, below));
                }
                Kind::Dir => {}
            }
        }
        Ok(Some(out))
    }

    /// `existsSync`: anything the Source finds, a folder as well as a file; `Missing` is `Err`, a refused path warns once for its folder (423).
    fn exists(&mut self, path: &str, folder: &str, file: &str) -> Result<bool, SourceError> {
        match self.source.stat(path) {
            Ok(_) => Ok(true),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(false),
            Err(SourceError::Outside) => {
                self.warn_outside(folder, file);
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// The files a module folder's `assets/` holds that a file param takes, sorted (424).
    fn assets_in(&mut self, folder: &str, spend: &mut Spend, file: &str) -> Walk<Arc<Vec<String>>> {
        if let Some(hit) = self.assets.get(folder) {
            return Ok(Some(hit.clone()));
        }
        let root = join(folder, "assets");
        let Some(all) = self.list_under(&root, ASSETS_DEPTH, spend, file)? else {
            return Ok(None);
        };
        let mut files: Vec<String> = all
            .into_iter()
            .filter(|p| {
                let lower = p.to_lowercase();
                ASSET_EXTS.iter().any(|e| lower.ends_with(e))
            })
            .collect();
        files.sort_by(|a, b| js_cmp(a, b));
        let files = Arc::new(files);
        self.assets.insert(folder.to_owned(), files.clone());
        Ok(Some(files))
    }

    /// The bases in one folder's `base/`, by name without the extension, subfolders as `<dir>/<name>`, sorted by code point.
    fn bases_in(&mut self, folder: &str, spend: &mut Spend, file: &str) -> Walk<Arc<Vec<String>>> {
        if let Some(hit) = self.bases.get(folder) {
            return Ok(Some(hit.clone()));
        }
        let Some(all) = self.list_under(&join(folder, "base"), BASES_DEPTH, spend, file)? else {
            return Ok(None);
        };
        let mut names: Vec<String> = all
            .into_iter()
            .filter_map(|f| {
                let lower = f.to_lowercase();
                BASE_EXTS
                    .iter()
                    .find(|e| lower.ends_with(**e))
                    .and_then(|e| f.get(..f.len() - e.len()).map(str::to_owned))
            })
            .collect();
        names.sort();
        let names = Arc::new(names);
        self.bases.insert(folder.to_owned(), names.clone());
        Ok(Some(names))
    }

    /// The shader folders directly under a module folder's `shaders/` that hold BOTH stages, sorted.
    fn shaders_in(&mut self, folder: &str, spend: &mut Spend, file: &str) -> Walk<Arc<Vec<String>>> {
        if let Some(hit) = self.shaders.get(folder) {
            return Ok(Some(hit.clone()));
        }
        let root = join(folder, "shaders");
        let mut names = Vec::new();
        for e in self.entries(&root, file)? {
            if e.kind != Kind::Dir {
                continue;
            }
            if !spend.charge(2) {
                return Ok(None);
            }
            let dir = join(&root, &e.name);
            if self.exists(&join(&dir, "vertex.glsl"), &root, file)?
                && self.exists(&join(&dir, "fragment.glsl"), &root, file)?
            {
                names.push(e.name);
            }
        }
        names.sort_by(|a, b| js_cmp(a, b));
        let names = Arc::new(names);
        self.shaders.insert(folder.to_owned(), names.clone());
        Ok(Some(names))
    }

    /// One ancestor resolved, once per ref: its folder and its `extends`, its findings dropped as the TypeScript's are (407).
    fn parent(&mut self, r: &str, spend: &mut Spend) -> Walk<(Option<String>, Option<String>)> {
        if let Some(known) = self.parents.get(r) {
            return Ok(known.as_ref().map(|p| (p.folder.clone(), p.extends.clone())));
        }
        let (resolved, _) = resolve_ref(self.source, self.roots, &self.inputs.vocabulary, r)?;
        let parent = resolved.as_ref().map(|m| Parent {
            folder: m.folder_abs.clone().filter(|f| !f.is_empty()),
            extends: m.authored.extends.clone().filter(|e| !e.is_empty()),
        });
        if let Some(m) = &resolved
            && !spend.charge(weigh(m))
        {
            return Ok(None);
        }
        let answer = parent.as_ref().map(|p| (p.folder.clone(), p.extends.clone()));
        self.parents.insert(r.to_owned(), parent);
        Ok(answer)
    }

    /// The folders to look in, nearest first: the module's own, then each ancestor's up to `MAX_ANCESTRY` levels (407).
    fn ancestry(&mut self, m: &GraphModule<'_>, spend: &mut Spend) -> Walk<Vec<String>> {
        let mut folders: Vec<String> = m.m.folder_abs.iter().filter(|f| !f.is_empty()).cloned().collect();
        let mut at = m.extends.map(str::to_owned);
        for _ in 0..MAX_ANCESTRY {
            let Some(r) = at.take() else { break };
            let Some((folder, extends)) = self.parent(&r, spend)? else {
                // Absent, or capped: the ancestry ends where the vocabulary does.
                if spend.capped() {
                    return Ok(None);
                }
                break;
            };
            folders.extend(folder);
            at = extends;
        }
        Ok(Some(folders))
    }

    /// Names gathered over the ancestry through `one`, each kept at its first place.
    fn gather(&mut self, m: &GraphModule<'_>, spend: &mut Spend, file: &str, one: Lister<'a>) -> Walk<Vec<String>> {
        let Some(folders) = self.ancestry(m, spend)? else {
            return Ok(None);
        };
        let (mut seen, mut out): (HashSet<String>, Vec<String>) = (HashSet::new(), Vec::new());
        for f in folders {
            let Some(list) = one(self, &f, spend, file)? else {
                return Ok(None);
            };
            for n in list.iter() {
                if seen.insert(n.clone()) {
                    out.push(n.clone());
                }
            }
        }
        Ok(Some(out))
    }

    /// The families whose folder a library root lists, read from one listing of the root.
    fn has_family(&mut self, root: &str, family: &str) -> Result<bool, SourceError> {
        if !self.families.contains_key(root) {
            let names = match self.source.list(root) {
                Ok(es) => Some(es.into_iter().filter(|e| e.kind == Kind::Dir).map(|e| e.name).collect()),
                Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => None,
                Err(e) => return Err(e),
            };
            self.families.insert(root.to_owned(), names);
        }
        Ok(self
            .families
            .get(root)
            .and_then(Option::as_ref)
            .is_some_and(|n| n.contains(family)))
    }

    /// Every file of a family: the project's own `@project/<path>` (samples and wavetables), then each library holding it, sorted (424).
    fn library_files(
        &mut self,
        family: &str,
        with_project: bool,
        spend: &mut Spend,
        file: &str,
    ) -> Walk<Arc<Vec<String>>> {
        let key = (family.to_owned(), with_project);
        if let Some(hit) = self.library.get(&key) {
            return Ok(Some(hit.clone()));
        }
        let mut roots: Vec<(String, String)> = Vec::new();
        if with_project {
            roots.push((PROJECT_SOUNDS.to_owned(), join(&self.inputs.project, family)));
        }
        let libs = self.roots.libraries.clone();
        for lib in libs {
            let root = crate::roots::spell(&lib.root, self.roots.paths)?;
            if self.has_family(&root, family)? {
                roots.push((lib.name, join(&root, family)));
            }
        }
        let mut out: Vec<String> = Vec::new();
        for (name, root) in roots {
            let Some(files) = self.list_under(&root, LIBRARY_DEPTH, spend, file)? else {
                return Ok(None);
            };
            for p in files {
                let leaf = p.rsplit('/').next().unwrap_or(&p);
                if is_library_file_of(family, leaf) {
                    out.push(format!("@{name}/{p}"));
                }
            }
        }
        let own = |r: &String| usize::from(!r.starts_with(&format!("@{PROJECT_SOUNDS}/")));
        out.sort_by(|a, b| own(a).cmp(&own(b)).then_with(|| js_cmp(a, b)));
        let out = Arc::new(out);
        self.library.insert(key, out.clone());
        Ok(Some(out))
    }
}

/// The library families a panel's pickers draw on: what the module asks for, the `library` a widget names, the file params it binds.
fn families_of(ui: Option<&ResolvedUi>, also: &[&'static str]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = also.to_vec();
    let Some(ui) = ui else { return out };
    let widgets = ui
        .widgets
        .values()
        .chain(ui.tabs.iter().flatten().flat_map(|t| t.widgets.values()));
    let mut named: Vec<&'static str> = Vec::new();
    let mut bound: Vec<&'static str> = Vec::new();
    for w in widgets {
        if let Some(l) = w.library.as_deref()
            && let Some((family, _)) = crate::modules::files::LIBRARY_FILE_FAMILIES
                .iter()
                .find(|(f, _)| *f == l)
        {
            named.push(family);
        }
        if let Some(b) = w.bind.as_ref() {
            if let Some((_, fam)) = LIBRARY_FILE_PARAMS.iter().find(|(p, _)| *p == b.0) {
                bound.push(fam);
            } else if let Some(fam) = shared_family_of(&b.0) {
                bound.push(fam);
            }
        }
    }
    for f in named.into_iter().chain(bound) {
        if !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// Fills a step's choices: assets, library files, bases, shaders and the amp model it loads; none when the budget stops a walk.
pub(super) fn fill(
    w: &mut Walks<'_>,
    s: &mut WebChainStep,
    m: &GraphModule<'_>,
    ui: Option<&ResolvedUi>,
    file: &str,
    spend: &mut Spend,
) -> Walk<()> {
    let own = w.gather(m, spend, file, Walks::assets_in)?;
    let Some(own) = own else { return Ok(None) };
    let rack = crate::resolve::resolved_type(Some(m.m)) == crate::resolve::InstrumentType::Rack;
    let families = families_of(ui, if rack { &["samples"] } else { &[] });
    let mut shared: Vec<String> = Vec::new();
    for family in families {
        let project = family == "samples" || family == "wavetables";
        let Some(list) = w.library_files(family, project, spend, file)? else {
            return Ok(None);
        };
        shared.extend(list.iter().cloned());
    }
    let choices: Vec<String> = own.into_iter().chain(shared).collect();
    s.asset_choices = (!choices.is_empty()).then_some(choices);
    let Some(bases) = w.gather(m, spend, file, Walks::bases_in)? else {
        return Ok(None);
    };
    s.bases = (!bases.is_empty()).then_some(bases);
    if m.category == Some("shaders") {
        let Some(shaders) = w.gather(m, spend, file, Walks::shaders_in)? else {
            return Ok(None);
        };
        s.shaders = (!shaders.is_empty()).then_some(shaders);
    }
    if let Some(path) = m.m.model.as_deref().filter(|p| !p.is_empty()) {
        let found = amp_model_info(w.source, path, &mut w.amp, spend)?;
        s.amp_model = found.map(|a| super::web::WebAmpModel {
            kind: a.kind.name().to_owned(),
            knobs: a.knobs,
        });
    }
    Ok(Some(()))
}
