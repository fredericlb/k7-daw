// modules/weight.rs — what a module COSTS to copy: one for itself and one for each entry of every list and map it holds.
// A reader would get wrong that the walk and the resolver share this: a limit that skips a list is bypassed through that list.

use super::{Alternative, LoadedModule, Module, Tab, Ui};
use indexmap::IndexMap;

fn tabs(t: &Option<Vec<Tab>>) -> usize {
    t.iter().flatten().map(|t| t.widgets.len()).sum()
}

fn ui(u: &Option<Ui>) -> usize {
    u.as_ref().map_or(0, |u| {
        u.widgets.len() + tabs(&u.tabs) + u.vertical.as_ref().map_or(0, |v| v.widgets.len() + tabs(&v.tabs))
    })
}

/// The lists and maps both a module and an alternative hold, one per entry, an engine's params and a rack's symbols included.
macro_rules! lists {
    ($m:expr) => {
        $m.params.as_ref().map_or(0, IndexMap::len)
            + ui(&$m.ui)
            + $m.rack.as_ref().map_or(0, |r| r.symbols.len())
            + $m.engines.as_ref().map_or(0, |e| {
                e.0.len()
                    + e.0
                        .values()
                        .map(|s| s.params.as_ref().map_or(0, IndexMap::len))
                        .sum::<usize>()
            })
            + $m.automations.as_ref().map_or(0, Vec::len)
            + $m.events.as_ref().map_or(0, Vec::len)
            + $m.tracks.as_ref().map_or(0, Vec::len)
            + $m.kit.as_ref().map_or(0, |k| k.0.len())
            + $m.tap_outs.as_ref().map_or(0, |k| k.0.len())
            + $m.inputs.as_ref().map_or(0, Vec::len)
            + $m.outputs.as_ref().map_or(0, Vec::len)
    };
}

fn alternative(a: &Alternative) -> usize {
    1 + lists!(a)
}

/// The entries of a module file: its lists and maps, and each alternative with its own.
pub fn module_entries(m: &Module) -> usize {
    lists!(m) + m.alternatives.iter().flatten().map(alternative).sum::<usize>()
}

/// The plugin's two words and its extra keys, which every node extending the module copies (decision 393).
fn plugin_text(m: &Module) -> usize {
    m.plugin.as_ref().map_or(0, |p| {
        p.plugin_type.as_deref().map_or(0, |t| t.len() / 16)
            + p.name.as_deref().map_or(0, |n| n.len() / 16)
            + p.extra.len()
    })
}

/// A loaded module as a merge copies it: itself, its authored entries, its assets, its map and its engine refs.
pub fn weigh(m: &LoadedModule) -> usize {
    1 + module_entries(&m.authored)
        + plugin_text(&m.authored)
        + m.assets.as_ref().map_or(0, IndexMap::len)
        + m.cc.as_ref().map_or(0, IndexMap::len)
        + m.engine_refs.as_ref().map_or(0, IndexMap::len)
}
