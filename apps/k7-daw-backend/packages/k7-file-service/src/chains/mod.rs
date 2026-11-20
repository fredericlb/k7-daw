//! chains — the account's chains read natively (1p): the three lists and the cut's chain, over one walk and one vocabulary.
//! What a reader gets wrong: a refusal is never an answer, every `Refused` of a template declines the whole request (1p-D9).

mod cold;
pub(crate) mod cold_run;
mod frozen;
mod live;
mod walk;

pub use cold::{ColdStep, cold_chains, cold_chains_view, cold_step_of};
pub use cold_run::{ColdRun, find_by, resolve_cold_run};
pub use frozen::frozen_chains;
pub use live::live_chains;
pub(crate) use live::refusal_at;
pub use walk::ChainDir;

use std::cell::OnceCell;

use serde_json::{Number, Value};
use song::Vocabulary;
use song::roots::Roots;
use song::source::Source;
use song::templates::{Refused, StepFacts, Template, TemplateStep, facts_of, read_template};

use crate::libraries::ACCOUNT_LIBRARY;
use crate::space::Declined;
use crate::tree::Limits;

/// The account's two chain settings, read from the preferences (1p-D12).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChainSettings {
    pub stems: Option<String>,
    pub instrumental: Option<String>,
}

/// One request's inputs: the source, the roots, the vocabulary built once, and the chains walked once (1p-D23).
pub struct Chains<'a> {
    pub(crate) source: &'a dyn Source,
    pub(crate) roots: &'a Roots,
    pub(crate) vocab: &'a song::Vocabulary,
    pub(crate) dirs: Vec<ChainDir>,
    read: Vec<OnceCell<Result<Template, Refused>>>,
}

/// What a refusal costs: the request is declined. A bound reached is `TooLarge`; any other refusal is what the TypeScript words.
pub(crate) fn declined(r: &Refused) -> Declined {
    match r {
        Refused::Limit => Declined::TooLarge,
        Refused::Native(_) => Declined::WouldThrow,
    }
}

impl<'a> Chains<'a> {
    /// The chains of every library `roots` names, walked once, each ref once, in code point order.
    pub fn new(
        source: &'a dyn Source,
        roots: &'a Roots,
        vocab: &'a Vocabulary,
        limits: &Limits,
    ) -> Result<Chains<'a>, Declined> {
        let dirs = walk::walk(roots, limits)?;
        let read = dirs.iter().map(|_| OnceCell::new()).collect();
        Ok(Chains {
            source,
            roots,
            vocab,
            dirs,
            read,
        })
    }

    /// The refs of the walk, in order.
    pub fn refs(&self) -> Vec<&str> {
        self.dirs.iter().map(|d| d.ref_.as_str()).collect()
    }

    /// Whether the account has cloned a library besides its own.
    pub fn cloned(&self) -> bool {
        self.roots
            .libraries
            .iter()
            .any(|l| l.name != ACCOUNT_LIBRARY)
    }

    /// The template of chain `i`, read once; a refusal declines.
    pub(crate) fn template(&self, i: usize) -> Result<&Template, Declined> {
        let d = &self.dirs[i];
        let read =
            self.read[i].get_or_init(|| read_template(self.source, &d.at(), self.roots.paths));
        read.as_ref().map_err(declined)
    }

    /// The sentence of chain `i`'s template when it does not read: such a chain is listed, not a reason to decline.
    pub(crate) fn unreadable(&self, i: usize) -> Option<String> {
        let d = &self.dirs[i];
        let read =
            self.read[i].get_or_init(|| read_template(self.source, &d.at(), self.roots.paths));
        match read {
            Err(Refused::Native(m)) => Some(m.clone()),
            _ => None,
        }
    }

    /// What a step is once resolved against the vocabulary.
    pub(crate) fn facts(&self, step: &TemplateStep) -> Result<StepFacts, Declined> {
        facts_of(self.source, self.roots, self.vocab, step).map_err(|r| declined(&r))
    }
}

/// A number as JavaScript prints it: an integer carries no fraction.
pub(crate) fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 && n != 0.0 {
        return Value::from(n as i64);
    }
    if n == 0.0 {
        return Value::from(0);
    }
    Number::from_f64(n).map_or(Value::Null, Value::Number)
}
