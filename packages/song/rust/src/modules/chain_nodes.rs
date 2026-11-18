// modules/chain_nodes.rs — a chain project's nodes: each item of `chains/`, a lane's from `tracks/`, keyed by lane name or id.
// A reader would get wrong that a preset lives in `vstpresets/` and is only stat'ed; a name that escapes it gives no preset at all.

use super::assets::{Files, Places};
use super::nodes::plugin_name_findings;
use super::preset::base_preset_in;
use super::types::{FAUST_FILE, FAUST_TYPE, JSDSP_FILE, JSDSP_TYPE};
use super::{LoadedModule, Module, ModuleKind};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::roots::{Roots, is_absolute, spell};
use crate::routing::{ChainProject, Item, lane_file};
use crate::source::path::{PathRule, join, join_within};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;

/// The folder a node's opaque plugin state lives in.
pub const VSTPRESETS_DIR: &str = "vstpresets";

/// A chain project's nodes, undecorated by the vocabulary, and what reading them found.
#[derive(Debug, Default)]
pub struct ChainNodes {
    pub nodes: IndexMap<String, LoadedModule>,
    pub findings: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

fn finding(file: &str, at: (u32, u32), message: String, severity: Severity) -> Finding {
    Finding {
        file: file.to_owned(),
        line: at.0,
        col: at.1,
        message,
        severity,
    }
}

/// `vstpresets/<name>` when the name stays inside the folder; a name that escapes, or is absolute, names nothing.
fn preset_name(presets: &str, name: &str, rule: PathRule) -> bool {
    // On Windows a `\` or a `:` is more than a name character, so it can climb out where `/` cannot.
    let hazard = rule.is_windows() && (name.contains('\\') || name.contains(':'));
    !hazard && !name.is_empty() && !is_absolute(name, rule) && join_within(presets, name).is_some_and(|p| p != presets)
}

/// The nodes of `project`: items and `tracks/` lanes as modules, their files resolved against the project folder.
pub fn load_chain_nodes(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    project: &ChainProject,
) -> Result<ChainNodes, SourceError> {
    let places = Places::of(roots, true)?;
    let dir = spell(&roots.project, roots.paths)?;
    let presets = join(&dir, VSTPRESETS_DIR);
    let mut out = ChainNodes::default();
    for n in project.nodes.values() {
        let (module, home, preset_name_of, label) = match &n.value {
            Item::VideoTrack { .. } => continue,
            Item::Track { track, at, .. } => {
                let label = format!("tracks/{track}.k7x");
                match cache.read::<Module>(source, &lane_file(&dir, track)) {
                    Ok(Ok(m)) => ((*m).clone(), "tracks", None, label),
                    Ok(Err(f)) => {
                        out.findings.push(f.in_file(&label));
                        continue;
                    }
                    Err(SourceError::NotFound) => {
                        let message = format!("lane \"{track}\" has no tracks/{track}.k7x");
                        out.findings.push(finding(
                            &format!("chains/{}.k7x", n.chain),
                            *at,
                            message,
                            Severity::Error,
                        ));
                        continue;
                    }
                    Err(SourceError::Outside) => {
                        let message = "outside the project's folders — skipped".to_owned();
                        out.warnings.push(finding(&label, (1, 1), message, Severity::Warning));
                        continue;
                    }
                    Err(SourceError::Io(_)) => continue,
                    Err(e) => return Err(e),
                }
            }
            Item::Node(item) => {
                let name = item
                    .vstpreset
                    .clone()
                    .unwrap_or_else(|| format!("{}.vstpreset", item.id));
                (
                    item.module.clone(),
                    "chains",
                    Some(name),
                    format!("chains/{}.k7x", n.chain),
                )
            }
        };
        if let Some(f) = plugin_name_findings(&label, &module) {
            out.findings.push(f);
        }
        let mut m = LoadedModule::bare(ModuleKind::Plugin, "", &n.name);
        m.category = None;
        m.kind = None;
        if module.io.is_some() {
            m.kind = Some(ModuleKind::Io);
            m.category = Some("io".to_owned());
        } else if module.extends.is_none() {
            m.kind = Some(ModuleKind::Plugin);
        }
        match module.plugin.as_ref().and_then(|p| p.plugin_type.as_deref()) {
            Some(JSDSP_TYPE) => m.jsdsp = Some(join(&dir, JSDSP_FILE)),
            Some(FAUST_TYPE) => m.faust = Some(join(&dir, FAUST_FILE)),
            _ => {}
        }
        m.authored = module;
        m.name = Some(n.name.clone());
        m.node = Some(n.name.clone());
        m.home = Some(home.to_owned());
        m.chain = Some(n.chain.clone());
        m.node_id = Some(n.id.clone());
        m.folder_abs = Some(dir.clone());
        let mut files = Files::new(source, &places, &m.own_label());
        if let Some(rack) = m.authored.rack.as_mut() {
            files.rack_audio(rack, &dir)?;
        }
        files.alternative_racks(&mut m.authored, &dir)?;
        // A chosen `base:` still wins over the node's own state.
        let chosen = match m.authored.base.clone() {
            Some(base) => base_preset_in(&mut files, &presets, &base)?,
            None => None,
        };
        let mut own = None;
        if let Some(name) = preset_name_of.filter(|name| preset_name(&presets, name, roots.paths)) {
            m.preset_rel = Some(format!("{VSTPRESETS_DIR}/{name}"));
            let path = join(&presets, &name);
            if files.is_file(&path)? {
                own = Some(path);
            }
        }
        m.preset = chosen.or(own);
        files.resolve_assets(&mut m, &dir)?;
        out.warnings.append(&mut files.findings);
        out.nodes.insert(n.name.clone(), m);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{PathRule, preset_name};

    #[test]
    fn a_windows_separator_or_colon_names_no_preset_there_only() {
        // Decision 100: `\` and `:` are characters on Unix and a way out on Windows.
        for name in ["..\\..\\x", "a\\b", "c:x", "a:b"] {
            assert!(!preset_name("/p/vstpresets", name, PathRule::Windows), "{name}");
            assert!(preset_name("/p/vstpresets", name, PathRule::Posix), "{name}");
        }
        assert!(preset_name("/p/vstpresets", "a.vstpreset", PathRule::Windows));
    }
}
