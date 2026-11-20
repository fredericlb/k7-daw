// templates/mod.rs — a library chain read as a template, and what each of its steps is once resolved (1p-D2).
// A reader would get wrong that a refusal is not an error answer: the host declines, and the TypeScript's sentence stands.

mod facts;
mod read;

pub use facts::{StepFacts, facts_of};
pub use read::read_template;

use crate::modules::Module;
use crate::routing::ChainInput;
use crate::source::SourceError;

/// A chain file's size past which it is not read (a limit, never a refusal of its content).
pub const MAX_CHAIN_BYTES: u64 = 8 << 20;

/// The steps a template may hold; past it the read stops with a limit.
pub const MAX_STEPS: usize = 50_000;

/// A folder chain's graph file, and the folder its node files live in.
pub const GRAPH_FILE: &str = "graph.k7x";
pub const NODES_DIR: &str = "nodes";

/// How a chain is kept on disk: one file, or the folder an unmigrated clone still holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    File,
    Folder,
}

/// One chain of a library as the walk found it: where it is, how, and the library it belongs to.
#[derive(Clone, Copy, Debug)]
pub struct ChainAt<'a> {
    pub at: &'a str,
    pub layout: Layout,
    pub library_root: &'a str,
}

/// One step as every reader runs it: a name for messages and the module fields a node file would hold.
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateStep {
    pub name: String,
    pub label: String,
    pub module: Module,
    pub bypass: bool,
    pub preset: Option<String>,
}

/// What a template reads as: its steps in signal order, and the steps it names but does not hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template {
    pub steps: Vec<TemplateStep>,
    pub missing: Vec<String>,
    pub input: Option<ChainInput>,
}

/// Why the native side does not answer: it declines, and the TypeScript's own sentence goes out (1p-D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The file or the resolution says something this reader will not word itself.
    Native(String),
    /// A bound was reached: the size of a file, the steps of a chain, a resolution's budget.
    Limit,
}

impl From<SourceError> for Refused {
    fn from(e: SourceError) -> Refused {
        Refused::Native(e.to_string())
    }
}
