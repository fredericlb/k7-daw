// modules/decorate.rs — what the tree and the nodes stamp on a module that reads more files: its audio, preset, assets, panel and map.
// A reader would get wrong that a panel REPLACES a module's inline `ui:` whole, on the built-ins and the catalogue as on the trees.

use super::assets::Files;
use super::panels::{Panels, module_own_refs, ui_weight};
use super::preset::preset_for;
use super::types::split_module_key;
use super::{LoadedModule, Vocabulary};
use crate::files::{Finding, Severity};
use crate::source::SourceError;
use indexmap::IndexMap;

/// Resolves the files a module names against its own folder: rack audio, the alternatives' racks, its preset, its file params.
pub fn stamp_files(
    files: &mut Files,
    lm: &mut LoadedModule,
    folder: &str,
    leaf: &str,
    node: Option<&str>,
) -> Result<(), SourceError> {
    if let Some(rack) = lm.authored.rack.as_mut() {
        files.rack_audio(rack, folder)?;
    }
    files.alternative_racks(&mut lm.authored, folder)?;
    let base = lm.authored.base.clone();
    lm.preset = preset_for(files, folder, leaf, node, base.as_deref())?;
    files.resolve_assets(lm, folder)
}

fn capped(file: &str, cap: usize) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message: format!("the panels stamped hold more than {cap} entries — the rest keep their own"),
        severity: Severity::Warning,
    }
}

/// Stamps each module with the panel and map of its ref minus the node; the file where the budget stopped it, if it did.
pub fn stamp_vocabulary(
    modules: &mut IndexMap<String, LoadedModule>,
    panels: &Panels,
    used: &mut usize,
    cap: usize,
) -> Option<String> {
    for (key, m) in modules.iter_mut() {
        let r = split_module_key(key).0;
        if let Some(ui) = panels.ui.get(r) {
            let w = ui_weight(ui);
            if *used + w > cap {
                return Some(format!("{r}/ui.k7x"));
            }
            *used += w;
            m.authored.ui = Some(ui.clone());
        }
        if let Some(map) = panels.maps.get(r) {
            if *used + map.len() > cap {
                return Some(format!("{r}/controller.k7x"));
            }
            *used += map.len();
            m.cc = Some(map.clone());
        }
    }
    None
}

/// Stamps a project's nodes from the vocabulary's tables, by the refs each may be filed under; no read of their own.
pub fn stamp_nodes(nodes: &mut IndexMap<String, LoadedModule>, vocab: &Vocabulary, cap: usize) -> Option<Finding> {
    let mut used = 0usize;
    for m in nodes.values_mut() {
        let refs = module_own_refs(m);
        if let Some(ui) = refs.iter().find_map(|r| vocab.panels.ui.get(r)) {
            used += ui_weight(ui);
            if used > cap {
                return Some(capped(&m.own_label(), cap));
            }
            m.authored.ui = Some(ui.clone());
        }
        if let Some(map) = refs.iter().find_map(|r| vocab.panels.maps.get(r)) {
            used += map.len();
            if used > cap {
                return Some(capped(&m.own_label(), cap));
            }
            m.cc = Some(map.clone());
        }
    }
    None
}
