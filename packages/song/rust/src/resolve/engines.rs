// resolve/engines.rs — a module's ENGINES: which one is on, and the module run as it, keeping its own identity.
// A reader would get wrong that an engine is not an alternative: the module declares engines, a node picks one, and `KEPT` stays the node's.

use super::{Resolver, weigh};
use crate::modules::assets::{Files, file_slot};
use crate::modules::files::{FILE_PARAMS, library_family_of};
use crate::modules::{EngineName, LoadedModule, ModuleType, ParamValue};
use crate::source::SourceError;
use indexmap::IndexMap;
use std::collections::HashSet;

/// The engine a module runs when it names none and has one called this: the Tracktion built-in.
const DEFAULT_ENGINE: &str = "tracktion";

/// What a module keeps of ITSELF when it runs as an engine: who it is and how it sits in the graph.
pub const KEPT: [&str; 22] = [
    "extends",
    "name",
    "node",
    "file",
    "home",
    "category",
    "status",
    "alternatives",
    "alternative",
    "engines",
    "engineRefs",
    "mix",
    "input_volume",
    "output_volume",
    "automations",
    "color",
    "disabled",
    "content",
    "frozen",
    "chain",
    "nodeId",
    "presetRel",
];

/// Every engine name a module offers, sorted, the default first.
pub fn engine_names(m: &LoadedModule) -> Vec<String> {
    let mut names: Vec<String> = m.engine_refs.iter().flat_map(IndexMap::keys).cloned().collect();
    names.sort();
    if let Some(at) = names.iter().position(|n| n == DEFAULT_ENGINE) {
        let default = names.remove(at);
        names.insert(0, default);
    }
    names
}

/// The engine that is ON: the one named if the module has it, else `tracktion`, else the first.
pub fn current_engine(m: &LoadedModule) -> Option<String> {
    let names = engine_names(m);
    let want = m.authored.engine.as_ref().map(|e| e.0.as_str());
    match want {
        Some(w) if names.iter().any(|n| n == w) => Some(w.to_owned()),
        _ => names.into_iter().next(),
    }
}

/// The params a module or node set for ONE engine, from its `engines:` map.
pub fn engine_params<'m>(m: &'m LoadedModule, engine: &str) -> Option<&'m IndexMap<String, ParamValue>> {
    m.authored.engines.as_ref()?.0.get(engine)?.params.as_ref()
}

impl Resolver<'_> {
    /// `chosen` run as its selected engine, the engine folded with its own ancestry; None when it has no engine to run as.
    pub(super) fn as_engine(
        &mut self,
        chosen: &LoadedModule,
        seen: &HashSet<String>,
    ) -> Result<Option<LoadedModule>, SourceError> {
        let Some(name) = current_engine(chosen) else {
            return Ok(None);
        };
        let Some(r) = chosen.engine_refs.as_ref().and_then(|refs| refs.get(&name)) else {
            return Ok(None);
        };
        let Some(engine) = self.module(r)? else {
            return Ok(None);
        };
        let Some(face) = self.fold_extends(&engine, seen, Some(r))? else {
            return Ok(None);
        };
        if !self.charge(weigh(&face)) {
            return Ok(None);
        }
        let mut out = face;
        let a = &chosen.authored;
        // KEPT drives what the module keeps of itself: a name it lists is taken from the module when the module has it.
        macro_rules! keep {
            ($k:expr; $($name:literal => $f:ident),*; $($cp:literal => $c:ident),*; $($own:literal => $o:ident),*) => {
                match $k {
                    $( $name => if a.$f.is_some() { out.authored.$f = a.$f.clone(); }, )*
                    $( $cp => if a.$c.is_some() { out.authored.$c = a.$c; }, )*
                    $( $own => if chosen.$o.is_some() { out.$o = chosen.$o.clone(); }, )*
                    "file" => out.file.clone_from(&chosen.file),
                    _ => {}
                }
            };
        }
        for k in KEPT {
            keep!(k;
                "extends" => extends, "status" => status, "alternatives" => alternatives, "alternative" => alternative,
                "engines" => engines, "automations" => automations, "content" => content, "frozen" => frozen,
                "color" => color;
                "mix" => mix, "input_volume" => input_volume, "output_volume" => output_volume, "disabled" => disabled;
                "name" => name, "node" => node, "home" => home, "category" => category, "engineRefs" => engine_refs,
                "chain" => chain, "nodeId" => node_id, "presetRel" => preset_rel);
        }
        // A node's taps are its KIT, kept on a rack engine and set aside on any other, which plays a kit of its own.
        if out.authored.module_type == Some(ModuleType::Rack) && a.rack.is_some() {
            out.authored.rack = a.rack.clone();
        }
        out.authored.engine = Some(EngineName(name.clone()));
        let set = engine_params(chosen, &name).cloned().unwrap_or_default();
        // The engine's defaults, then what this module or node set for THAT engine, never its top-level params.
        let mut params = out.authored.params.take().unwrap_or_default();
        params.extend(set.clone());
        out.authored.params = Some(params);
        // …and a FILE it set there is the file the engine loads, not the engine's default one.
        let folders = [out.folder_abs.clone(), chosen.folder_abs.clone()];
        self.engine_files(&mut out, &set, &folders)?;
        Ok(Some(out))
    }

    /// The files a set of engine params names, resolved as the loader resolves a module's own; their names join the engine's.
    fn engine_files(
        &mut self,
        out: &mut LoadedModule,
        set: &IndexMap<String, ParamValue>,
        folders: &[Option<String>],
    ) -> Result<(), SourceError> {
        let refs: Vec<Option<&str>> = folders.iter().map(Option::as_deref).collect();
        let mut files = Files::new(self.source, self.places, &self.at);
        let mut assets: IndexMap<String, String> = IndexMap::new();
        for (k, v) in set {
            let ParamValue::Text(text) = v else { continue };
            if text.is_empty() {
                continue;
            }
            let found = if FILE_PARAMS.contains(&k.as_str()) {
                files.file_param_in(k, text, &refs)?
            } else if let Some(family) = library_family_of(k) {
                files.library_file(family, text)?
            } else {
                continue;
            };
            *file_slot(out, k) = found;
            assets.insert(k.clone(), text.clone());
        }
        self.findings.append(&mut files.findings);
        if !assets.is_empty() {
            let mut all = out.assets.take().unwrap_or_default();
            all.extend(assets);
            out.assets = Some(all);
        }
        Ok(())
    }
}
