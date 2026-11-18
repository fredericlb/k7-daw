// routing/levels.rs — a chain project's link `level`s laid onto the derived chains' `out[].gain`, the send's level.
// A reader would get wrong that only a link to a BUS can be played at a level; the rest are named, not dropped.

use super::split_port;
use super::types::Chain;
use indexmap::IndexMap;

/// The node a chain's outs leave from: its last step, else its source.
fn last_node(c: &Chain) -> Option<&str> {
    c.use_steps.last().map(|s| s.module.as_str()).or(c.from.as_deref())
}

/// Each out's gain from `levels` (keyed `<from>><to>`), in place.
pub fn apply_link_levels(chains: &mut IndexMap<String, Chain>, levels: &IndexMap<String, f64>) {
    if levels.is_empty() {
        return;
    }
    for c in chains.values_mut() {
        let Some(from) = last_node(c).map(str::to_owned) else {
            continue;
        };
        for o in &mut c.out {
            if let Some(level) = levels.get(&format!("{from}>{}", o.to)) {
                o.gain = *level;
            }
        }
    }
}

/// A level other than unity on a link the engine document has no gain for.
#[derive(Clone, Debug, PartialEq)]
pub struct Unplayed {
    pub chain: String,
    pub to: String,
    pub gain: f64,
}

/// Is `to` a bus the compile sends to: a chain of its own, entered by its main input, fed by no lane.
fn is_bus_target(
    chains: &IndexMap<String, Chain>,
    to: &str,
    is_sink: &dyn Fn(&str) -> bool,
    is_lane: &dyn Fn(&str) -> bool,
) -> bool {
    if split_port(to).1.is_some() || is_sink(to) {
        return false;
    }
    chains.get(to).is_some_and(|c| !c.from.as_deref().is_some_and(is_lane))
}

/// The links whose level is not played, since only a link to a bus carries one.
pub fn unplayed_levels(
    chains: &IndexMap<String, Chain>,
    is_sink: &dyn Fn(&str) -> bool,
    is_lane: &dyn Fn(&str) -> bool,
) -> Vec<Unplayed> {
    let mut out = Vec::new();
    for (name, c) in chains {
        for o in &c.out {
            if o.gain != 1.0 && !is_bus_target(chains, &o.to, is_sink, is_lane) {
                out.push(Unplayed {
                    chain: name.clone(),
                    to: o.to.clone(),
                    gain: o.gain,
                });
            }
        }
    }
    out
}
