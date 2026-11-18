// lanes/inputs.rs — what a compile reads of a project, together and once: head, routing, nodes, the resolved modules, the three families.
// A reader would get wrong that only `Missing` is an `Err`: every other trouble with a file is a finding, and `stops()` says which end a compile.

use super::rig::Rig;
use crate::cache::{Cache, FileValue};
use crate::clips::{AudioPaths, audio_paths};
use crate::files::{Clip, Finding, Sequence, Severity, Timeline};
use crate::lanes::{Lanes, lanes_of};
use crate::modules::{NodesRead, Vocabulary, load_nodes};
use crate::read::{FamilyRead, Head, read_family, read_head};
use crate::resolve::{Resolution, Resolved, resolve_map};
use crate::roots::{Roots, spell};
use crate::routing::{
    Chain, RoutedTap, Routing, apply_link_levels, derive_chains, lane_names, load_routing, tap_pairs,
    with_chain_module_refs,
};
use crate::source::path::PathRule;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

/// Everything a compile reads off a project, read once; the rest of the compile is pure over this.
pub struct Inputs {
    pub rig: Rig,
    pub project: String,
    /// The path rule of the roots this was read under.
    pub rule: PathRule,
    pub head: Head,
    pub routing: Routing,
    pub nodes: NodesRead,
    pub vocabulary: Arc<Vocabulary>,
    /// The project's nodes, resolved: what decides which nodes are lanes (244).
    pub local: Resolution,
    /// The vocabulary modules the chains name, resolved apart (245).
    pub extra: Resolution,
    pub chains: IndexMap<String, Chain>,
    pub bypassed: HashSet<String>,
    pub bus_only: HashSet<String>,
    pub routed_taps: IndexMap<String, Vec<RoutedTap>>,
    pub clips: FamilyRead<Clip>,
    pub sequences: FamilyRead<Sequence>,
    pub timelines: FamilyRead<Timeline>,
    pub audio_paths: AudioPaths,
    /// Every WARNING the reads raised, none of which stops a compile.
    pub warnings: Vec<Finding>,
}

impl Inputs {
    /// A module by the name a chain step gives it: a node of the project, else a vocabulary module the chains name.
    pub fn module(&self, name: &str) -> Option<&Resolved> {
        self.local.modules.get(name).or_else(|| self.extra.modules.get(name))
    }

    /// Whether a step's module is a live input (`io: in`).
    pub fn is_input(&self, name: &str) -> bool {
        self.module(name).is_some_and(|m| io_is_in(m))
    }

    /// The project's lanes, decided on its own nodes (244).
    pub fn lanes(&self) -> Lanes<'_> {
        lanes_of(&self.local.modules)
    }

    /// The findings that end a compile (249): the errors of the head, the routing, the nodes and both resolves, and of `timelines/`.
    pub fn stops(&self) -> Vec<&Finding> {
        let heads = self.head.findings.iter().chain(&self.head.compile_findings);
        let others = self
            .routing
            .findings
            .iter()
            .chain(&self.nodes.findings)
            .chain(&self.local.findings)
            .chain(&self.extra.findings)
            .chain(&self.timelines.findings);
        heads.chain(others).filter(|f| f.severity == Severity::Error).collect()
    }
}

/// Whether the module is an `io: in`.
fn io_is_in(m: &crate::modules::LoadedModule) -> bool {
    m.authored.io == Some(crate::modules::IoKind::In)
}

/// A family folder the Source refuses as a whole is one warning named by its folder and an empty family (247).
pub fn family_or_warning<T: FileValue>(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
) -> Result<FamilyRead<T>, SourceError> {
    match read_family::<T>(source, cache, project) {
        Err(SourceError::Outside) => Ok(FamilyRead {
            values: Vec::new(),
            names: Vec::new(),
            findings: vec![Finding {
                file: T::DIR.to_owned(),
                line: 1,
                col: 1,
                message: "outside the project's folders — skipped".to_owned(),
                severity: Severity::Warning,
            }],
        }),
        other => other,
    }
}

/// Reads a project the way the TypeScript's `loadCompileInputs` does, in its order (246); `Err` only for `Missing`.
pub fn load_inputs(source: &dyn Source, cache: &mut Cache, roots: &Roots, rig: Rig) -> Result<Inputs, SourceError> {
    cache.set_paths(roots.paths);
    let project = spell(&roots.project, roots.paths)?;
    let head = read_head(source, cache, &project)?;
    let routing = load_routing(source, cache, roots)?;
    let nodes = load_nodes(source, cache, roots)?;
    let vocabulary = cache.vocabulary(source, roots)?;
    let local = resolve_map(source, roots, &nodes.nodes, &vocabulary.modules)?;
    let names = lane_names(&local.modules);
    let mut chains = derive_chains(&routing.segments, &|n| names.contains(n));
    apply_link_levels(&mut chains, &routing.levels);
    let bypassed: HashSet<String> = routing.segments.bypassed_of().map(str::to_owned).collect();
    let bus_only: HashSet<String> = routing.segments.bus_only_of().map(str::to_owned).collect();
    let routed_taps = tap_pairs(routing.segments.tap_feeds_of());
    let clips = family_or_warning::<Clip>(source, cache, &project)?;
    let sequences = family_or_warning::<Sequence>(source, cache, &project)?;
    let timelines = family_or_warning::<Timeline>(source, cache, &project)?;
    let paths = audio_paths(source, &project, &clips, roots.paths)?;
    // The refs of the chains alone are asked of the vocabulary; a name a node already holds is the node's.
    let named = with_chain_module_refs(&IndexMap::new(), &vocabulary.modules, &chains);
    let named: IndexMap<_, _> = named
        .into_iter()
        .filter(|(k, _)| !nodes.nodes.contains_key(k))
        .collect();
    let extra = resolve_map(source, roots, &named, &vocabulary.modules)?;
    let mut warnings: Vec<Finding> = nodes.warnings.clone();
    let reads = [
        &head.findings,
        &head.compile_findings,
        &routing.findings,
        &nodes.findings,
        &vocabulary.findings,
        &local.findings,
        &extra.findings,
        &clips.findings,
        &sequences.findings,
        &timelines.findings,
    ];
    warnings.extend(
        reads
            .into_iter()
            .flatten()
            .filter(|f| f.severity == Severity::Warning)
            .cloned(),
    );
    Ok(Inputs {
        rig,
        project,
        rule: roots.paths,
        head,
        routing,
        nodes,
        vocabulary,
        local,
        extra,
        chains,
        bypassed,
        bus_only,
        routed_taps,
        clips,
        sequences,
        timelines,
        audio_paths: paths,
        warnings,
    })
}
