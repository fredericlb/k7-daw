// views/hints/params.rs — the params each base module offers: its own, its built-in plugin's, and its extenders' (decision 628).
// A reader would get wrong that the extenders are indexed ONCE over the nodes: a scan per base is a product of two big lists.

use super::{Hints, ParamRange};
use crate::lanes::inputs::Inputs;
use crate::modules::builtin_plugin;
use crate::views::Spend;
use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;

/// What each base's extenders have written on it, in one pass; false once the budget ends the pass.
fn extenders<'a>(inputs: &'a Inputs, spend: &mut Spend) -> Option<HashMap<&'a str, IndexSet<&'a str>>> {
    let mut by: HashMap<&str, IndexSet<&str>> = HashMap::new();
    for node in inputs.nodes.nodes.values() {
        if !spend.step() {
            return None;
        }
        let Some(base) = node.authored.extends.as_deref().filter(|e| !e.is_empty()) else {
            continue;
        };
        let keys = by.entry(base).or_default();
        for k in node.authored.params.iter().flat_map(IndexMap::keys) {
            if !spend.step() {
                return None;
            }
            keys.insert(k.as_str());
        }
    }
    Some(by)
}

/// `paramsByBase` and `paramMetaByBase` for the base refs given; stops where the budget does.
pub fn params_by_base(hints: &mut Hints, inputs: &Inputs, bases: &[String], spend: &mut Spend) {
    let Some(by_extends) = extenders(inputs, spend) else {
        return;
    };
    for r in bases {
        let Some(base) = inputs.vocabulary.modules.get(r) else {
            continue;
        };
        let own = base.authored.params.as_ref();
        let plugin = base.authored.plugin.as_ref();
        let builtin = plugin.and_then(|p| builtin_plugin(p.plugin_type.as_deref(), p.name.as_deref()));
        let extra = by_extends.get(r.as_str());
        // The work is charged before it is done: the sets below are as long as these three together.
        let work =
            own.map_or(0, IndexMap::len) + builtin.map_or(0, |b| b.params.len()) + extra.map_or(0, IndexSet::len);
        if !spend.steps(work) {
            return;
        }
        let mut keys: IndexSet<&str> = own.into_iter().flat_map(IndexMap::keys).map(String::as_str).collect();
        if let Some(b) = builtin {
            let mut meta = IndexMap::new();
            for p in &b.params {
                keys.insert(p.name.as_str());
                if !spend.bytes(p.name.len()) {
                    return;
                }
                meta.insert(
                    p.name.clone(),
                    ParamRange {
                        min: p.min,
                        max: p.max,
                        def: p.def,
                    },
                );
            }
            hints.param_meta_by_base.insert(r.clone(), meta);
        }
        keys.extend(extra.into_iter().flatten().copied());
        if keys.is_empty() {
            continue;
        }
        let mut names: Vec<&str> = keys.into_iter().collect();
        names.sort_unstable();
        let mut out = Vec::with_capacity(names.len());
        for n in names {
            if !spend.bytes(n.len()) {
                // What was paid for stays: the base lists the names it got to.
                hints.params_by_base.insert(r.clone(), out);
                return;
            }
            out.push(n.to_owned());
        }
        hints.params_by_base.insert(r.clone(), out);
    }
}
