// routing/refs.rs — the modules a chain step may name: the project's own, plus only those SHARED refs the chains USE.
// A reader would get wrong that a ref arrives from both a step's `module` and an out's `to`, or it works only as a step.

use super::types::Chain;
use crate::modules::LoadedModule;
use indexmap::IndexMap;

/// `nodes` plus each vocabulary module a chain names that no node already is, as it stands (resolving is the caller's, once).
pub fn with_chain_module_refs(
    nodes: &IndexMap<String, LoadedModule>,
    by_ref: &IndexMap<String, LoadedModule>,
    chains: &IndexMap<String, Chain>,
) -> IndexMap<String, LoadedModule> {
    let mut out = nodes.clone();
    for c in chains.values() {
        let names = c
            .use_steps
            .iter()
            .map(|s| s.module.as_str())
            .chain(c.out.iter().map(|o| o.to.as_str()));
        for name in names {
            if name.is_empty() || out.contains_key(name) {
                continue;
            }
            if let Some(m) = by_ref.get(name) {
                out.insert(name.to_owned(), m.clone());
            }
        }
    }
    out
}
