// views/catalogue.rs — the module picker's two lists: the shared modules a project can name, and the bases to extend.
// A reader would get wrong that `path` is ABSOLUTE and an engine module has none: the host relativizes it.

use super::{Spend, capped_finding};
use crate::files::Finding;
use crate::modules::types::split_module_key;
use crate::modules::{LoadedModule, ModuleKind, Vocabulary, base_module_refs};
use crate::roots::{Roots, spell};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use serde::Serialize;
use std::collections::HashMap;

/// The grid size of a module's panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Panel {
    pub cols: u32,
    pub rows: u32,
}

/// A module's panel grid, only when both sides are above 0; none for a module that draws no panel.
pub fn panel_of(m: &LoadedModule) -> Option<Panel> {
    let ui = m.authored.ui.as_ref()?;
    let side = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    let (cols, rows) = (side(ui.cols.0), side(ui.rows.0));
    (cols > 0 && rows > 0).then_some(Panel { cols, rows })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct CatalogueModule {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub name: String,
    pub namespace: String,
    pub category: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub extends: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub path: Option<String>,
    pub nodes: u32,
    pub has_ui: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub panel: Option<Panel>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BaseModuleRef {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub panel: Option<Panel>,
}

/// A list a view answers, what it found while listing, and whether its budget stopped it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Listed<T> {
    pub items: Vec<T>,
    pub findings: Vec<Finding>,
    pub capped: bool,
    pub spent: usize,
}

fn kind_name(kind: Option<ModuleKind>) -> &'static str {
    match kind {
        Some(ModuleKind::Io) => "io",
        Some(ModuleKind::Plugin) => "plugin",
        Some(ModuleKind::Doc) => "doc",
        Some(ModuleKind::Track) => "track",
        None => "",
    }
}

/// The shared roots of the catalogue: the module roots without `@meta`, a later root of a namespace winning.
fn roots_by_namespace(roots: &Roots) -> Result<HashMap<String, String>, SourceError> {
    let mut list = roots.module_roots()?;
    if let Some(meta) = &roots.meta
        && list
            .last()
            .is_some_and(|r| r.namespace == "meta" && Some(&r.dir) == spell(meta, roots.paths).ok().as_ref())
    {
        list.pop();
    }
    Ok(list.into_iter().map(|r| (r.namespace, r.dir)).collect())
}

/// Every generic module the project inherits, in ref order; one whose file cannot be opened is dropped.
pub fn module_catalogue(
    source: &dyn Source,
    roots: &Roots,
    v: &Vocabulary,
) -> Result<Listed<CatalogueModule>, SourceError> {
    let by_ns = roots_by_namespace(roots)?;
    // The nodes are counted once, not listed: they share their generic's row.
    let mut nodes: HashMap<&str, u32> = HashMap::new();
    for key in v.modules.keys() {
        let (module, node) = split_module_key(key);
        if node.is_some() {
            *nodes.entry(module).or_insert(0) += 1;
        }
    }
    let mut spend = Spend::new();
    let mut items: Vec<CatalogueModule> = Vec::new();
    for (r, m) in &v.modules {
        if !spend.step() {
            break;
        }
        if !r.starts_with('@') || split_module_key(r).1.is_some() || m.engine_of.is_some() {
            continue;
        }
        let parts: Vec<&str> = r[1..].split('/').collect();
        let (ns, rest) = (parts[0], &parts[1..]);
        let path = match by_ns.get(ns) {
            Some(root) => {
                let mut dir = root.clone();
                for seg in rest {
                    dir = join(&dir, seg);
                }
                let file = join(&dir, "module.k7x");
                match source.stat(&file) {
                    Ok(s) if s.kind == Kind::File => Some(file),
                    Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => continue,
                    Err(e) => return Err(e),
                }
            }
            None if rest.len() == 1 => None,
            None => continue,
        };
        let name = rest.join("/");
        let plugin = m
            .authored
            .plugin
            .as_ref()
            .and_then(|p| p.name.clone())
            .filter(|n| !n.is_empty());
        let extends = m.authored.extends.clone().filter(|e| !e.is_empty());
        let weight =
            r.len() + name.len() + path.as_ref().map_or(0, String::len) + plugin.as_ref().map_or(0, String::len);
        if !spend.bytes(weight) {
            break;
        }
        items.push(CatalogueModule {
            ref_: r.clone(),
            name,
            namespace: ns.to_owned(),
            category: m
                .category
                .clone()
                .unwrap_or_else(|| rest.first().map_or(String::new(), |s| (*s).to_owned())),
            kind: kind_name(m.kind).to_owned(),
            plugin,
            extends,
            path,
            nodes: nodes.get(r.as_str()).copied().unwrap_or(0),
            has_ui: m.authored.ui.is_some(),
            panel: panel_of(m),
        });
    }
    items.sort_by(|a, b| a.ref_.cmp(&b.ref_));
    let findings = if spend.capped() {
        vec![capped_finding("project.k7x", "the module catalogue")]
    } else {
        Vec::new()
    };
    Ok(Listed {
        items,
        findings,
        capped: spend.capped(),
        spent: spend.spent(),
    })
}

/// The plugin refs a new module can extend, each with its panel; sorted.
pub fn base_modules(v: &Vocabulary) -> Listed<BaseModuleRef> {
    let mut spend = Spend::new();
    let mut items = Vec::new();
    for r in base_module_refs(&v.modules) {
        if !spend.step() || !spend.bytes(r.len()) {
            break;
        }
        let panel = v.modules.get(&r).and_then(panel_of);
        items.push(BaseModuleRef { ref_: r, panel });
    }
    let findings = if spend.capped() {
        vec![capped_finding("project.k7x", "the base module list")]
    } else {
        Vec::new()
    };
    Listed {
        items,
        findings,
        capped: spend.capped(),
        spent: spend.spent(),
    }
}
