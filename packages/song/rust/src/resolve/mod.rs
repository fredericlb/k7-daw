// resolve/mod.rs — "what does this module run with?": its ancestry folded, its alternative applied, the engine it runs as.
// A reader would get wrong that `Resolved` has no public constructor: a consumer cannot take a raw module by accident.

mod alternatives;
mod engines;
mod extends;
mod findings;
mod view;

pub use alternatives::{
    AlternativeInfo, MIXER_PLUGIN, alternatives_of, current_alternative, keeps_its_module, pick_alternative,
    swapped_module,
};
pub use engines::{KEPT, current_engine, engine_names, engine_params};
pub use extends::OWN_ONLY;
pub use findings::{not_supported_findings, plugin_findings, unknown_plugin_findings};
pub use view::{
    InstrumentType, ResolvedLayout, ResolvedRack, ResolvedUi, resolved_cc, resolved_rack, resolved_type, resolved_ui,
};

use crate::files::{Finding, Severity};
use crate::modules::assets::{Files, Places, mentions_project};
use crate::modules::types::split_module_key;
use crate::modules::{LoadedModule, Vocabulary};
use crate::roots::Roots;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;

/// What one call to resolve may produce: a resolved module weighs one and each entry a merge copies one.
pub const BUDGET: usize = 1_000_000;

/// How many `extends` levels a fold goes up before it stops.
pub const MAX_DEPTH: usize = 32;

/// A module with its ancestry folded in; only this module makes one, so a raw module is never taken for it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Resolved(LoadedModule);

impl Deref for Resolved {
    type Target = LoadedModule;

    fn deref(&self) -> &LoadedModule {
        &self.0
    }
}

impl Resolved {
    /// The module without the brand, for a consumer that is done with resolving.
    pub fn into_inner(self) -> LoadedModule {
        self.0
    }
}

/// A map resolved, keys unchanged, and what resolving found.
#[derive(Debug, Default)]
pub struct Resolution {
    pub modules: IndexMap<String, Resolved>,
    pub findings: Vec<Finding>,
    /// Whether the budget stopped the call: the module it stopped at and every later one are absent.
    pub capped: bool,
    /// What the call spent against its budget.
    pub weight: usize,
}

/// A fold kept for the rest of the call: the module folded, the folders of its whole ancestry (nearest first) and the refs it stands on.
#[derive(Clone)]
pub(crate) struct Kept {
    pub module: Arc<LoadedModule>,
    pub folders: Arc<Vec<Option<String>>>,
    /// The ref folded and every ancestor under it: a reuse needs them off the caller's path and within the depth left.
    pub refs: Arc<Vec<String>>,
}

pub(crate) struct Resolver<'a> {
    pub(crate) by_ref: &'a IndexMap<String, LoadedModule>,
    pub(crate) source: &'a dyn Source,
    pub(crate) places: &'a Places,
    pub(crate) findings: Vec<Finding>,
    weight: usize,
    budget: usize,
    folded: HashMap<String, Kept>,
    pub(crate) capped: bool,
    /// The file a finding of the module being resolved is named by.
    pub(crate) at: String,
}

pub(crate) use crate::modules::weight::weigh;

impl<'a> Resolver<'a> {
    pub(crate) fn new(
        source: &'a dyn Source,
        places: &'a Places,
        by_ref: &'a IndexMap<String, LoadedModule>,
        budget: usize,
    ) -> Resolver<'a> {
        Resolver {
            by_ref,
            source,
            places,
            findings: Vec::new(),
            weight: 0,
            budget,
            folded: HashMap::new(),
            capped: false,
            at: String::new(),
        }
    }

    /// Spends `n`; false (and one error at the module being resolved) once the budget is passed.
    pub(crate) fn charge(&mut self, n: usize) -> bool {
        self.weight = self.weight.saturating_add(n);
        if self.weight <= self.budget {
            return true;
        }
        if !self.capped {
            self.capped = true;
            let message = format!(
                "resolving holds more than {} entries — this module and the rest are left out",
                self.budget
            );
            self.finding(Severity::Error, message);
        }
        false
    }

    pub(crate) fn finding(&mut self, severity: Severity, message: String) {
        self.findings.push(Finding {
            file: self.at.clone(),
            line: 1,
            col: 1,
            message,
            severity,
        });
    }

    /// A vocabulary module as a fold meets it: an `@project/` file it names resolved against this compile's project.
    pub(crate) fn module(&mut self, r: &str) -> Result<Option<Cow<'a, LoadedModule>>, SourceError> {
        let Some(m) = self.by_ref.get(r) else { return Ok(None) };
        Ok(Some(self.with_project(m)?))
    }

    pub(crate) fn with_project<'b>(&mut self, m: &'b LoadedModule) -> Result<Cow<'b, LoadedModule>, SourceError> {
        if self.places.project.is_none() || !mentions_project(m) {
            return Ok(Cow::Borrowed(m));
        }
        let mut own = m.clone();
        let mut files = Files::new(self.source, self.places, &self.at);
        files.project_pass(&mut own)?;
        self.findings.append(&mut files.findings);
        Ok(Cow::Owned(own))
    }

    /// One module resolved, or None when the budget stops it.
    fn resolve_one(&mut self, key: &str, m: &LoadedModule) -> Result<Option<Resolved>, SourceError> {
        let own_module = self.with_project(m)?;
        let seen: HashSet<String> = std::iter::once(key.to_owned()).collect();
        let Some(own) = self.fold_extends(&own_module, &seen, self.by_ref.contains_key(key).then_some(key))? else {
            return Ok(None);
        };
        // A set naming another module makes the node that module, except a lane or a fader.
        let swapped = if keeps_its_module(&own) {
            None
        } else {
            self.swapped(&own_module)
        };
        let chosen = match swapped {
            Some(s) => match self.fold_extends(&s, &seen, None)? {
                Some(c) => c,
                None => return Ok(None),
            },
            None => self.apply_alternative(own),
        };
        if self.capped {
            return Ok(None);
        }
        // A module with ENGINES runs as the one that is on, once the set has said which.
        let done = self.as_engine(&chosen, &seen)?.unwrap_or(chosen);
        Ok((!self.capped && self.charge(1)).then_some(Resolved(done)))
    }
}

/// The file a finding about the module of `key` is named by: a project node's file, or the ref's own.
fn label_of(key: &str, m: &LoadedModule) -> String {
    if matches!(m.home.as_deref(), Some("nodes" | "chains" | "tracks")) {
        return m.own_label();
    }
    let (module, node) = split_module_key(key);
    format!("{module}/{}.k7x", node.unwrap_or("module"))
}

/// Every module of `modules` resolved against `by_ref`, keys unchanged; `by_ref` is the vocabulary a node extends into.
pub fn resolve_map(
    source: &dyn Source,
    roots: &Roots,
    modules: &IndexMap<String, LoadedModule>,
    by_ref: &IndexMap<String, LoadedModule>,
) -> Result<Resolution, SourceError> {
    resolve_map_with(source, roots, modules, by_ref, BUDGET)
}

/// The same, with the budget named.
pub fn resolve_map_with(
    source: &dyn Source,
    roots: &Roots,
    modules: &IndexMap<String, LoadedModule>,
    by_ref: &IndexMap<String, LoadedModule>,
    budget: usize,
) -> Result<Resolution, SourceError> {
    let places = Places::of(roots, true)?;
    let mut r = Resolver::new(source, &places, by_ref, budget);
    let mut out = IndexMap::new();
    for (key, m) in modules {
        r.at = label_of(key, m);
        match r.resolve_one(key, m)? {
            Some(done) => {
                out.insert(key.clone(), done);
            }
            None => break,
        }
    }
    Ok(Resolution {
        modules: out,
        findings: r.findings,
        capped: r.capped,
        weight: r.weight,
    })
}

/// A ref resolved against the vocabulary it lives in; None when nothing answers to it.
pub fn resolve_ref(
    source: &dyn Source,
    roots: &Roots,
    vocab: &Vocabulary,
    module_ref: &str,
) -> Result<(Option<Resolved>, Vec<Finding>), SourceError> {
    let places = Places::of(roots, true)?;
    let mut r = Resolver::new(source, &places, &vocab.modules, BUDGET);
    let Some(m) = vocab.modules.get(module_ref) else {
        return Ok((None, Vec::new()));
    };
    r.at = label_of(module_ref, m);
    let done = r.resolve_one(module_ref, m)?;
    Ok((done, r.findings))
}
