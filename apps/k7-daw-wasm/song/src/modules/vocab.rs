// modules/vocab.rs — every module a project can name by REF: the built-ins, the engine's catalogue, then each root's tree.
// A reader would get wrong the order: a later root overwrites an earlier one's ref but the key keeps its first place.

use super::assets::Places;
use super::decorate::stamp_vocabulary;
use super::panels::{Panels, read_panels};
use super::scan::{Scan, engine_modules, io_modules};
use super::tree::{ModuleFolder, read_tree_in};
use super::types::{
    COLD_TYPE, CONSOLE_TYPE, DOC_CATEGORY, DOC_REF, NOT_SUPPORTED_CATEGORY, NOT_SUPPORTED_REF, NOT_SUPPORTED_TYPE,
    TRACK_CATEGORY, TRACK_REFS, ports, type_meta,
};
use super::{LoadedModule, ModuleKind, Plugin, Port, PortType};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::roots::{Roots, spell};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;

/// The vocabulary holds at most this many modules: past it a walk stops with one warning naming its root.
pub const MAX_ENTRIES: usize = 100_000;

/// What the roots offer, by ref, and what was learned reading them.
#[derive(Debug)]
pub struct Vocabulary {
    pub modules: IndexMap<String, LoadedModule>,
    /// Every module folder walked, nearest root first: where 2c looks `ui.k7x` and `controller.k7x` up.
    pub folders: Vec<ModuleFolder>,
    /// The warnings of the files that did not read, by file.
    pub findings: Vec<Finding>,
    /// What the engine's scan said; no plugins means it cannot say and the built-ins stand in.
    pub scan: Scan,
    /// The panels and MIDI maps found beside the module folders, by ref, already stamped on the modules.
    pub panels: Panels,
    /// The files of a module or node that did not decode, as named in `findings`, sorted.
    pub broken: Vec<String>,
}

/// Why a ref answers nothing: no root offers it, or its file is there and did not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Absent {
    Unknown,
    Broken { file: String },
}

impl Vocabulary {
    /// What an `extends` of `module_ref` that no module answers to means: a broken file of that ref, else nothing known.
    pub fn absent(&self, module_ref: &str) -> Absent {
        // `broken` is sorted: the files under `<ref>/` are one run, found by binary search (decision 265).
        let prefix = format!("{module_ref}/");
        let at = self.broken.partition_point(|f| f.as_str() < prefix.as_str());
        match self.broken.get(at) {
            Some(file) if file.starts_with(&prefix) => Absent::Broken { file: file.clone() },
            _ => Absent::Unknown,
        }
    }
}

fn port(name: &str, kind: PortType) -> Port {
    Port {
        name: name.to_owned(),
        kind,
        mandatory: None,
    }
}

/// The doc module, the four lanes, the not-supported placeholder and the io roles: no file anywhere.
pub fn builtin_modules() -> IndexMap<String, LoadedModule> {
    let mut out = IndexMap::new();
    if let Some(meta) = type_meta(DOC_CATEGORY) {
        let mut m = LoadedModule::bare(meta.kind, DOC_CATEGORY, "doc");
        m.authored.inputs = Some(ports(meta.inputs));
        m.authored.outputs = Some(ports(meta.outputs));
        out.insert(DOC_REF.to_owned(), m);
    }
    for r in TRACK_REFS {
        let file = r.rsplit('/').next().unwrap_or(r);
        let mut m = LoadedModule::bare(ModuleKind::Track, TRACK_CATEGORY, file);
        m.authored.inputs = Some(Vec::new());
        m.authored.outputs = Some(match r {
            "@meta/group-track" => Vec::new(),
            "@meta/picture-track" => vec![port("video", PortType::Video), port("audio", PortType::Audio)],
            _ => vec![port("audio", PortType::Audio), port("midi", PortType::Midi)],
        });
        m.generated = (r == "@meta/strudel-track").then_some(true);
        m.picture = (r == "@meta/picture-track").then_some(true);
        out.insert(r.to_owned(), m);
    }
    let mut ns = LoadedModule::bare(ModuleKind::Plugin, NOT_SUPPORTED_CATEGORY, "not-supported");
    ns.authored.inputs = Some(Vec::new());
    ns.authored.outputs = Some(vec![port("out", PortType::Audio)]);
    ns.authored.plugin = Some(Plugin {
        plugin_type: Some(NOT_SUPPORTED_TYPE.to_owned()),
        ..Plugin::default()
    });
    out.insert(NOT_SUPPORTED_REF.to_owned(), ns);
    out.extend(io_modules());
    out
}

/// The vocabulary under `roots`, with the module cap at `cap`.
pub fn build_vocabulary_with(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    cap: usize,
) -> Result<Vocabulary, SourceError> {
    let scan_path = roots.scan.as_deref().map(|d| spell(d, roots.paths)).transpose()?;
    let scan = Scan::read(source, scan_path.as_deref())?;
    let mut modules = builtin_modules();
    modules.extend(engine_modules(&scan));
    let (mut folders, mut findings, mut broken) = (Vec::new(), Vec::new(), Vec::new());
    let places = Places::of(roots, false)?;
    let module_roots = roots.module_roots()?;
    for root in &module_roots {
        let tree = read_tree_in(source, cache, root, cap.saturating_sub(modules.len()), &places)?;
        modules.extend(tree.modules);
        folders.extend(tree.folders);
        findings.extend(tree.findings);
        broken.extend(tree.broken);
    }
    let mut used = modules.len();
    let mut panels = read_panels(source, cache, &folders, used, cap)?;
    findings.append(&mut panels.findings);
    used += panels.weight;
    if let Some(file) = stamp_vocabulary(&mut modules, &panels, &mut used, cap) {
        findings.push(Finding {
            file,
            line: 1,
            col: 1,
            message: format!("the panels stamped hold more than {cap} entries — the rest keep their own"),
            severity: Severity::Warning,
        });
    }
    broken.sort();
    Ok(Vocabulary {
        modules,
        folders,
        findings,
        scan,
        panels,
        broken,
    })
}

/// The vocabulary under `roots`, read in the order the TypeScript reads: built-ins, catalogue, then each root.
pub fn build_vocabulary(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<Vocabulary, SourceError> {
    build_vocabulary_with(source, cache, roots, MAX_ENTRIES)
}

/// The plugin refs a new module can `extends`: no io module, no node, no engine, no console half, no cold module; sorted.
pub fn base_module_refs(modules: &IndexMap<String, LoadedModule>) -> Vec<String> {
    let mut refs: Vec<&String> = modules
        .iter()
        .filter(|(_, m)| {
            m.kind == Some(ModuleKind::Plugin)
                && m.node.is_none()
                && m.engine_of.is_none()
                && !matches!(m.category.as_deref(), Some(CONSOLE_TYPE | COLD_TYPE))
        })
        .map(|(r, _)| r)
        .collect();
    // JavaScript's default sort: by UTF-16 units.
    refs.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    refs.into_iter().cloned().collect()
}
