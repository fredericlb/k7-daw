// lanes/ownership.rs — which engine track each chain's steps land on: its lane, a bus return, the master, a lane-less input, a tap's run.
// A reader would get wrong that all four owned kinds are named after the CHAIN, not after a lane; the master's owner is `@master`.
// Part of 2h's compile (docs/nodeProject.md); the decisions are 252 to 256 of docs/rustCompilerLanes.md.

use super::inputs::Inputs;
use super::live::input_landing_track;
use super::references::{Fault, FaultKind, Holder, MAX_FAULTS, held};
use super::rig::resolve_cue_output;
use super::{CUE_NODE, Lanes, MASTER_OWNER, OUT_NODE, is_builtin_output};
use crate::modules::ModuleKind;
use crate::routing::{Chain, MIX_NODE};
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

/// A run a routed tap feeds: the rack node it comes from and the output pair it leaves by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapChain {
    pub node: String,
    pub pair: u32,
}

/// Which track every chain contributes to, and the runs that own a track of their own.
#[derive(Debug, Default)]
pub struct Ownership {
    pub cue_pair: Vec<u32>,
    pub bus_chains: IndexSet<String>,
    pub input_chains: IndexSet<String>,
    pub tap_chains: IndexMap<String, TapChain>,
    pub faults: Vec<Fault>,
    owners: HashMap<String, String>,
    cued: HashSet<String>,
    chains: HashSet<String>,
    lanes: HashSet<String>,
    // The chain holding a step, by its position in `names`: a step never copies its chain's name (283).
    holder: HashMap<String, usize>,
    names: Vec<String>,
}

impl Ownership {
    /// The track a chain feeds; None is the TypeScript's `""` (252).
    pub fn owner_of(&self, chain: &str) -> Option<&str> {
        self.owners.get(chain).map(String::as_str)
    }

    /// Whether some `out` of the chain leaves for the CUE bus.
    pub fn is_cued(&self, chain: &str) -> bool {
        self.cued.contains(chain)
    }

    /// The track a step's side input comes from: a chain's owner, else a lane's own name, else the owner of the chain holding the step (255).
    pub fn side_input_track(&self, id: &str) -> Option<&str> {
        if self.chains.contains(id) {
            return self.owner_of(id);
        }
        if let Some(lane) = self.lanes.get(id) {
            return Some(lane);
        }
        let at = *self.holder.get(id)?;
        self.owner_of(self.names.get(at)?)
    }
}

/// Everything a chain's `from` can be asked at once: the roles the TypeScript's closures play.
struct Roles<'a> {
    inputs: &'a Inputs,
    lanes: &'a Lanes<'a>,
}

impl Roles<'_> {
    /// The five built-in outputs end a run by NAME; without them a run ending at VIDEO_OUT is taken for a bus.
    fn is_sink(&self, to: &str) -> bool {
        to.is_empty() || is_builtin_output(to) || self.inputs.module(to).is_some_and(|m| m.kind == Some(ModuleKind::Io))
    }

    fn is_track_fed(&self, c: &Chain) -> bool {
        c.from.as_deref().is_some_and(|f| self.lanes.sound.contains_key(f))
    }

    fn is_master(c: &Chain) -> bool {
        c.from.as_deref() == Some(MIX_NODE) && c.out.iter().any(|o| o.to == OUT_NODE)
    }
}

/// Which track every chain contributes to (docs/nodeProject.md); a tap routed to no subchain is a fault, not a throw (254).
pub fn chain_ownership(inputs: &Inputs, lanes: &Lanes<'_>) -> Ownership {
    let roles = Roles { inputs, lanes };
    let chains = &inputs.chains;
    let mut own = Ownership {
        cue_pair: resolve_cue_output(&inputs.rig).channels,
        ..Ownership::default()
    };
    // A non-sink target naming a chain not fed by a lane is a bus; a picture lane never is, though a fade's stretch routes into its run.
    for c in chains.values() {
        for o in &c.out {
            if !roles.is_sink(&o.to)
                && chains.get(&o.to).is_some_and(|t| !roles.is_track_fed(t))
                && !lanes.picture.contains_key(o.to.as_str())
            {
                own.bus_chains.insert(o.to.clone());
            }
        }
    }
    for (name, c) in chains {
        if c.out.iter().any(|o| o.to == CUE_NODE) {
            own.cued.insert(name.clone());
        }
        // A laneless run carrying a live input owns a track, unless it lands on a lane, which is the ordinary recording shape.
        if Roles::is_master(c) || roles.is_track_fed(c) || own.bus_chains.contains(name) {
            continue;
        }
        if !c.use_steps.iter().any(|s| inputs.is_input(&s.module)) {
            continue;
        }
        if input_landing_track(c, &|id| lanes.sound.contains_key(id)).is_none() {
            own.input_chains.insert(name.clone());
        }
    }
    // A run fed by a routed tap owns a track too: the instrument's pair is returned at its front (255).
    let mut first_run: HashMap<&str, &str> = HashMap::new();
    for (name, c) in chains {
        if c.from.is_none()
            && let Some(step) = c.use_steps.first()
        {
            first_run.entry(step.module.as_str()).or_insert(name.as_str());
        }
    }
    for list in inputs.routed_taps.values() {
        for t in list {
            if let Some(run) = first_run.get(t.to.as_str()) {
                own.tap_chains.insert(
                    (*run).to_owned(),
                    TapChain {
                        node: t.node.clone(),
                        pair: t.pair,
                    },
                );
            }
        }
    }
    own.faults = unrouted_taps(inputs);
    // Every chain's owner, computed once (252).
    for (name, c) in chains {
        let owner = if Roles::is_master(c) {
            Some(MASTER_OWNER)
        } else if roles.is_track_fed(c)
            || own.bus_chains.contains(name)
            || own.input_chains.contains(name)
            || own.tap_chains.contains_key(name)
        {
            Some(name.as_str())
        } else {
            None
        };
        own.chains.insert(name.clone());
        if let Some(o) = owner {
            own.owners.insert(name.clone(), o.to_owned());
        }
        let at = own.names.len();
        own.names.push(name.clone());
        for s in &c.use_steps {
            if !own.holder.contains_key(&s.module) {
                own.holder.insert(s.module.clone(), at);
            }
        }
    }
    own.lanes = lanes.sound.keys().map(|k| (*k).to_owned()).collect();
    own
}

/// Every tap a node routes must have reached a run: one that did not is a fault at its node.
fn unrouted_taps(inputs: &Inputs) -> Vec<Fault> {
    let routed: HashMap<&str, HashSet<&str>> = inputs
        .routed_taps
        .iter()
        .map(|(node, list)| (node.as_str(), list.iter().map(|t| t.tap.as_str()).collect()))
        .collect();
    let mut faults = Vec::new();
    let modules = inputs.local.modules.iter().chain(&inputs.extra.modules);
    for (node, m) in modules {
        let Some(outs) = m.authored.tap_outs.as_ref() else {
            continue;
        };
        for tap in outs.0.keys() {
            if !routed.get(node.as_str()).is_some_and(|set| set.contains(tap.as_str())) {
                if faults.len() >= MAX_FAULTS {
                    faults.push(Fault::new(FaultKind::More, Holder::File(String::new()), ""));
                    return faults;
                }
                faults.push(Fault::new(FaultKind::TapUnrouted, Holder::Node(held(node)), tap));
            }
        }
    }
    faults
}
