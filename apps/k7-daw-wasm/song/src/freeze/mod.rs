// freeze/mod.rs — the freeze graph: what is upstream of a module, and whether one render can stand in for it.
// A reader would get wrong that the walk is bounded by a Budget: a None is "the budget is spent", never "nothing found".

mod auto;
mod cut;
mod fingerprint;

pub use auto::{
    AutoFreeze, AutoFreezes, AutoInput, FreezePlugin, auto_freeze_points, auto_freezes, auto_render_path,
    is_vst_module, render_ready,
};
pub use cut::freeze_cut;
pub use fingerprint::{MAX_DIGEST_BYTES, freeze_fingerprint};

use crate::clips::Budget;
use crate::routing::{Chain, split_port};
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// What one compile's freezes may spend: an edge or node walked, a track cut, a value hashed, a stat (decision 348).
pub const BUDGET: usize = 3_000_000;

/// What copying one name costs the freeze budget: one unit and a unit per 64 bytes (decision 348).
pub(crate) fn name_units(name: &str) -> usize {
    1 + name.len() / 64
}

/// What a freeze at a module does to the document: the lanes it empties and the modules it takes out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreezePlan {
    pub at: String,
    pub render: String,
    pub lanes: IndexSet<String>,
    pub modules: IndexSet<String>,
}

/// Why a module cannot be frozen; the wording belongs to whoever shows it (decision 337).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Blocked {
    NoSuchModule,
    FeedsOutside { from: String, to: String },
}

/// Every edge of the derived chains, once, between nodes (a lane or a step), borrowed from the chains.
pub struct Edges<'c> {
    edges: Vec<(&'c str, &'c str)>,
    back: HashMap<&'c str, Vec<&'c str>>,
    nodes: HashSet<&'c str>,
    steps: HashSet<&'c str>,
    names: HashSet<&'c str>,
}

impl<'c> Edges<'c> {
    /// The edges of `chains` as `freeze.ts` reads them: lane into step, step into step, the last into each out, each `in` into its step.
    pub fn of(chains: &'c IndexMap<String, Chain>) -> Edges<'c> {
        let mut e = Edges {
            edges: Vec::new(),
            back: HashMap::new(),
            nodes: HashSet::new(),
            steps: HashSet::new(),
            names: HashSet::new(),
        };
        for c in chains.values() {
            let from = c.from.as_deref();
            let path = from.into_iter().chain(c.use_steps.iter().map(|s| s.module.as_str()));
            let mut prev: Option<&'c str> = None;
            for n in path {
                if let Some(p) = prev {
                    e.push(p, n);
                }
                prev = Some(n);
            }
            if let Some(f) = from {
                e.nodes.insert(f);
                e.names.insert(f);
            }
            for s in &c.use_steps {
                e.nodes.insert(&s.module);
                e.names.insert(&s.module);
                e.steps.insert(&s.module);
            }
            if let Some(last) = prev {
                for o in &c.out {
                    e.push(last, split_port(&o.to).0);
                }
            }
            for s in &c.use_steps {
                for src in s.inputs.iter().flatten() {
                    e.push(split_port(src).0, &s.module);
                }
            }
        }
        e
    }

    fn push(&mut self, from: &'c str, to: &'c str) {
        self.edges.push((from, to));
        self.back.entry(to).or_default().push(from);
        self.names.insert(from);
        self.names.insert(to);
    }

    /// How many edges there are.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Is `node` a lane or a step the chains actually name (an out's target alone is not)?
    pub fn has_node(&self, node: &str) -> bool {
        self.nodes.contains(node)
    }

    /// Is `module` a step of some run?
    pub fn on_a_chain(&self, module: &str) -> bool {
        self.steps.contains(module)
    }

    /// Every node whose signal reaches `module`, itself included; a name in no edge has none here. None: the budget is spent.
    pub fn upstream_of(&self, module: &str, budget: &mut Budget) -> Option<IndexSet<&'c str>> {
        let mut seen: IndexSet<&'c str> = IndexSet::new();
        let Some(&start) = self.names.get(module) else {
            return Some(seen);
        };
        seen.insert(start);
        let mut stack = vec![start];
        while let Some(at) = stack.pop() {
            let froms = self.back.get(at).map_or(&[][..], Vec::as_slice);
            if !budget.charge(1 + froms.len()) {
                return None;
            }
            for &from in froms {
                if seen.insert(from) {
                    stack.push(from);
                }
            }
        }
        Some(seen)
    }

    /// Why `module` cannot be frozen, given its upstream set: the outer None is a spent budget, the inner None is "it can".
    pub fn blocked_by(&self, module: &str, inside: &IndexSet<&'c str>, budget: &mut Budget) -> Option<Option<Blocked>> {
        if inside.len() <= 1 && !self.has_node(module) {
            return Some(Some(Blocked::NoSuchModule));
        }
        for &(from, to) in &self.edges {
            if !budget.charge(1) {
                return None;
            }
            if from != module && inside.contains(from) && !inside.contains(to) {
                return Some(Some(Blocked::FeedsOutside {
                    from: from.to_owned(),
                    to: to.to_owned(),
                }));
            }
        }
        Some(None)
    }
}

/// The plan of every frozen `(module, render)` the chains name; None: the budget is spent.
pub fn freeze_plans(
    edges: &Edges<'_>,
    frozen_at: &[(String, String)],
    is_lane: &dyn Fn(&str) -> bool,
    budget: &mut Budget,
) -> Option<Vec<FreezePlan>> {
    let mut plans = Vec::new();
    for (at, render) in frozen_at {
        if !budget.charge(1) {
            return None;
        }
        if !edges.has_node(at) {
            continue;
        }
        let inside = edges.upstream_of(at, budget)?;
        // Every name the plan copies is paid by its bytes BEFORE the copy.
        let copies = name_units(at) + name_units(render) + inside.iter().map(|n| name_units(n)).sum::<usize>();
        if !budget.charge(copies) {
            return None;
        }
        let mut lanes = IndexSet::new();
        let mut modules = IndexSet::new();
        for n in inside {
            if is_lane(n) {
                lanes.insert(n.to_owned());
            } else {
                modules.insert(n.to_owned());
            }
        }
        plans.push(FreezePlan {
            at: at.clone(),
            render: render.clone(),
            lanes,
            modules,
        });
    }
    Some(plans)
}

/// The module half of a compiled plugin id, `<chain>:<module>`.
pub(crate) fn plugin_module(id: &str) -> &str {
    id.find(':').map_or(id, |i| &id[i + 1..])
}
