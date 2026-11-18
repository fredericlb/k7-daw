// modules/nodes.rs — a project's `nodes/`: each file a node keyed by its name, and the one plugin rule that blocks a compile.
// A reader would get wrong that a chain project's nodes are not `nodes/` files: they are chain items (chain_nodes.rs).

use super::assets::{Files, Places};
use super::chain_nodes::load_chain_nodes;
use super::decorate::{stamp_files, stamp_nodes};
use super::types::{FAUST_FILE, FAUST_TYPE, JSDSP_FILE, JSDSP_TYPE, KNOWN_PLUGIN_TYPES};
use super::vocab::MAX_ENTRIES;
use super::{LoadedModule, Module, ModuleKind};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::read::read_family;
use crate::roots::Roots;
use crate::routing::read_chain_project;
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexMap;

/// What a project's `nodes/` holds.
#[derive(Debug, Default)]
pub struct NodesRead {
    /// Keyed by the bare file name, undecorated so inheritance can fill it.
    pub nodes: IndexMap<String, LoadedModule>,
    /// A file that did not decode, and a known plugin format with no name: errors, one per file, located.
    pub findings: Vec<Finding>,
    /// Warnings of the files a node names: one the Source refuses, panels past the budget.
    pub warnings: Vec<Finding>,
    /// A project with `chains/`: its nodes are the chain items and the lanes' files.
    pub chain_project: bool,
}

/// A known plugin format must name its plugin: the one plugin rule that blocks a compile.
pub fn plugin_name_findings(file: &str, module: &Module) -> Option<Finding> {
    let plugin = module.plugin.as_ref()?;
    let kind = plugin.plugin_type.as_deref().unwrap_or("").to_lowercase();
    if !KNOWN_PLUGIN_TYPES.contains(&kind.as_str()) || plugin.name.as_deref().is_some_and(|n| !n.is_empty()) {
        return None;
    }
    let (line, col) = module.plugin_at.unwrap_or((1, 1));
    Some(Finding {
        file: file.to_owned(),
        line,
        col,
        message: "plugin.name is required — for \"internal\" the built-in's xmlTypeName (e.g. \"4osc\"), else the external plugin's name"
            .to_owned(),
        severity: Severity::Error,
    })
}

/// A project's nodes as `loadModules` answers: read, their files resolved, their panel and map stamped from the vocabulary.
pub fn load_nodes(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<NodesRead, SourceError> {
    let places = Places::of(roots, true)?;
    let project = crate::roots::spell(&roots.project, roots.paths)?;
    let mut out = read_nodes_in(source, cache, &project, &places)?;
    if out.chain_project {
        let cp = read_chain_project(source, cache, &project)?;
        let cn = load_chain_nodes(source, cache, roots, &cp)?;
        for f in cp.findings.into_iter().chain(cn.findings) {
            if f.severity == Severity::Error {
                out.findings.push(f)
            } else {
                out.warnings.push(f)
            }
        }
        out.warnings.extend(cn.warnings);
        out.nodes = cn.nodes;
    }
    if !out.nodes.is_empty() {
        let vocab = cache.vocabulary(source, roots)?;
        out.warnings.extend(stamp_nodes(&mut out.nodes, &vocab, MAX_ENTRIES));
    }
    Ok(out)
}

/// Reads `nodes/` of a project; `kind` and `category` are stamped as the loader does, the files resolved against `places`.
pub(crate) fn read_nodes_in(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
    places: &Places,
) -> Result<NodesRead, SourceError> {
    let mut out = NodesRead::default();
    match source.stat(&join(project, "chains")) {
        Ok(st) if st.kind == Kind::Dir => {
            out.chain_project = true;
            return Ok(out);
        }
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => {}
        Err(e) => return Err(e),
    }
    let fam = match read_family::<Module>(source, cache, project) {
        Ok(f) => f,
        Err(SourceError::Outside) => {
            out.findings.push(Finding {
                file: "nodes".to_owned(),
                line: 1,
                col: 1,
                message: "outside the project's folders — skipped".to_owned(),
                severity: Severity::Warning,
            });
            return Ok(out);
        }
        Err(e) => return Err(e),
    };
    out.findings = fam.findings;
    let folder = join(project, "nodes");
    for ((node, value), name) in fam.values.into_iter().zip(fam.names) {
        let mut m = LoadedModule::bare(ModuleKind::Plugin, "", &node);
        m.authored = (*value).clone();
        m.category = None;
        m.kind = None;
        // A node without `extends` has no folder to take `kind` from: io by its `io:`, else a plugin.
        if m.authored.io.is_some() {
            m.kind = Some(ModuleKind::Io);
            m.category = Some("io".to_owned());
        } else if m.authored.extends.is_none() {
            m.kind = Some(ModuleKind::Plugin);
        }
        m.name = Some(node.clone());
        m.node = Some(node.clone());
        m.home = Some("nodes".to_owned());
        match value.plugin.as_ref().and_then(|p| p.plugin_type.as_deref()) {
            Some(JSDSP_TYPE) => m.jsdsp = Some(join(&folder, JSDSP_FILE)),
            Some(FAUST_TYPE) => m.faust = Some(join(&folder, FAUST_FILE)),
            _ => {}
        }
        m.folder_abs = Some(folder.clone());
        if let Some(f) = plugin_name_findings(&format!("nodes/{name}"), &value) {
            out.findings.push(f);
        }
        let mut files = Files::new(source, places, &format!("nodes/{name}"));
        stamp_files(&mut files, &mut m, &folder, &node, Some(&node))?;
        out.warnings.extend(files.findings);
        out.nodes.insert(node, m);
    }
    Ok(out)
}
