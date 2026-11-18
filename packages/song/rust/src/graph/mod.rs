// graph/mod.rs — the module graph the Chains tab draws: its model, its validation and its warnings (sub-step 2k, Part A).
// A reader would get wrong that nothing here compiles: a graph stands on the `Inputs` a compile loaded, and what it
// refuses is its own rule, a little narrower than the compile's (404, 405). docs/rustCompilerGraph.md.

mod build;
pub mod index;
pub mod issues;
pub mod meter;
pub mod model;
pub mod place;
pub mod reach;
pub mod strip;
pub mod validate;
mod validate_lanes;
pub mod view;

pub use index::Index;
pub use issues::{GraphIssue, WarningCode, issue};
pub use model::{
    Channel, ClipKind, GraphModule, GraphPlacement, InputDef, OutputDef, Track, TrackType, VideoLane, graph_module,
};
pub use place::{NodePlace, node_places, node_stacks};
pub use strip::{lane_strip, lane_strips_of, picture_lanes};
pub use validate::{validate_inner, validate_issues};
pub use view::{
    Answer, AutoMark, ClipExtent, Folder, FreezeMark, Handed, ModelNeed, handed_of, load_compiled_graph,
    load_web_graph, model_need, render_exists, web_graph,
};

use crate::cache::Cache;
use crate::files::Finding;
use crate::lanes::inputs::Inputs;
use crate::lanes::references::Fault;
use crate::roots::Roots;
use crate::routing::{Chain, Position, TapFeed};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;

/// Units of 64 bytes of output, or one step of a walk, one graph may spend (419).
pub const BUDGET: usize = 1_000_000;
/// Issues one graph keeps; the sentences outside `validate_issues` count under the same cap (410, 411).
pub const MAX_ISSUES: usize = 1_000;
/// Characters of a name a warning quotes.
pub const NAME_CUT: usize = 200;
/// Levels of `extends` an ancestry walk climbs.
pub const MAX_ANCESTRY: usize = 16;

/// What one graph has spent against `BUDGET`.
#[derive(Clone, Debug, Default)]
pub struct Spend {
    used: usize,
    capped: bool,
}

impl Spend {
    pub fn new() -> Spend {
        Spend::default()
    }

    /// Spends `units`; false, and capped for good, once the budget is passed.
    pub fn charge(&mut self, units: usize) -> bool {
        if self.capped {
            return false;
        }
        self.used = self.used.saturating_add(units);
        if self.used > BUDGET {
            self.capped = true;
        }
        !self.capped
    }

    /// Spends the weight of `bytes` of output: one unit and a unit per 64 bytes.
    pub fn bytes(&mut self, bytes: usize) -> bool {
        self.charge(1 + bytes / 64)
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn capped(&self) -> bool {
        self.capped
    }
}

/// A project as the graph reads it, borrowed from the `Inputs` it stands on.
pub struct GraphDoc<'i> {
    pub chains: IndexMap<String, Chain>,
    pub tracks: IndexMap<&'i str, Track<'i>>,
    pub video_tracks: IndexMap<&'i str, VideoLane<'i>>,
    pub disabled: Vec<&'i str>,
    pub modules: IndexMap<&'i str, GraphModule<'i>>,
    pub positions: Option<&'i IndexMap<String, Position>>,
    pub assemblies: IndexMap<String, crate::assemblies::Assembly>,
    pub tap_feeds: &'i [TapFeed],
    pub issues: Vec<GraphIssue>,
    pub warnings: Vec<String>,
    /// What the TypeScript lost in silence or threw away; never `validate`'s output (445).
    pub findings: Vec<Finding>,
}

/// Why a graph is not drawn: the findings that stop it, and the wiring faults.
#[derive(Debug)]
pub struct Refusal {
    pub findings: Vec<Finding>,
    pub faults: Vec<Fault>,
}

#[derive(Debug)]
pub enum GraphError {
    Source(SourceError),
    Refused(Refusal),
}

impl From<SourceError> for GraphError {
    fn from(e: SourceError) -> GraphError {
        GraphError::Source(e)
    }
}

/// The graph of a project whose inputs a compile loaded (401); `Refused` when the TypeScript would throw (405).
pub fn load_graph<'i>(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    inputs: &'i Inputs,
) -> Result<GraphDoc<'i>, GraphError> {
    build::load(source, cache, roots, inputs)
}
