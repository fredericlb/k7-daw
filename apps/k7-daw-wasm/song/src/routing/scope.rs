// routing/scope.rs — SUBCHAINS: a chain file's own chains, reached only from inside that file, keyed `<parent>/<id>`.
// A reader would get wrong that a link resolves inside its parent's scope first, and a subchain reaches only a sibling or a built-in end.

use super::chain_file::{CHAIN_SINKS, ChainFile, Step};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The key a subchain is known by in memory.
pub fn sub_key(parent: &str, sub: &str) -> String {
    format!("{parent}/{sub}")
}

/// A key split back into its file chain and subchain id; a top-level key has no `sub`.
pub fn split_key(key: &str) -> (&str, Option<&str>) {
    match key.split_once('/') {
        Some((p, s)) => (p, Some(s)),
        None => (key, None),
    }
}

/// A chain as the compile walks it: a file's own steps, or one of its subchains under a scoped key.
#[derive(Clone, Debug)]
pub struct ChainUnit<'a> {
    pub key: String,
    /// The FILE's chain: itself for a top-level chain.
    pub parent: &'a str,
    /// The subchain's id inside `parent`, absent for the file's own steps.
    pub sub: Option<&'a str>,
    pub file: &'a ChainFile,
    pub steps: &'a [Step],
}

impl ChainUnit<'_> {
    /// The built-in end or chain a unit's end step links to, none for a unit with no end step.
    pub fn links(&self) -> &[super::chain_file::Link] {
        self.steps.last().and_then(Step::end).map_or(&[], |e| e.to.as_slice())
    }
}

/// Every chain of the project with a per-file index of its subchain ids.
pub struct Scope<'a> {
    pub chains: &'a IndexMap<String, Arc<ChainFile>>,
    pub units: Vec<ChainUnit<'a>>,
    by_key: HashMap<String, usize>,
    subs: HashMap<&'a str, HashSet<&'a str>>,
}

/// Every chain of the project, each file's subchains right after it.
pub fn chain_units(chains: &IndexMap<String, Arc<ChainFile>>) -> Scope<'_> {
    let mut units = Vec::new();
    let mut by_key = HashMap::new();
    let mut subs: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (name, c) in chains {
        by_key.insert(name.clone(), units.len());
        units.push(ChainUnit {
            key: name.clone(),
            parent: name,
            sub: None,
            file: c,
            steps: &c.steps,
        });
        let ids = subs.entry(name.as_str()).or_default();
        for sc in c.subchains.iter().flatten() {
            ids.insert(sc.id.as_str());
            by_key.insert(sub_key(name, &sc.id), units.len());
            units.push(ChainUnit {
                key: sub_key(name, &sc.id),
                parent: name,
                sub: Some(&sc.id),
                file: c,
                steps: &sc.steps,
            });
        }
    }
    Scope {
        chains,
        units,
        by_key,
        subs,
    }
}

impl<'a> Scope<'a> {
    pub fn unit(&self, key: &str) -> Option<&ChainUnit<'a>> {
        self.by_key.get(key).map(|i| &self.units[*i])
    }

    /// What a link out of `from` names: a sibling subchain first, then (from a file's own steps) a chain or an end.
    pub fn resolve_link(&self, from: &ChainUnit, name: &str) -> Option<String> {
        if self.subs.get(from.parent).is_some_and(|s| s.contains(name)) {
            return Some(sub_key(from.parent, name));
        }
        let sink = CHAIN_SINKS.contains(&name);
        if sink && !self.chains.contains_key(name) {
            return Some(name.to_owned());
        }
        // Closed both ways: a subchain reaches nothing outside its parent but a built-in end.
        if from.sub.is_some() {
            return sink.then(|| name.to_owned());
        }
        self.chains.contains_key(name).then(|| name.to_owned())
    }
}
