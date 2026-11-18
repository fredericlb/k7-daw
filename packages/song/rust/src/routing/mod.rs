// routing/mod.rs — where the signal goes: both project formats read into ONE segment graph, and the runs derived from it.
// A reader would get wrong that nothing here compiles: it reads, and every later stage walks what it derives (docs/rustCompiler.md).

mod chain_check;
pub mod chain_file;
pub mod derive;
pub mod graph_file;
mod hand;
pub mod layout;
pub mod levels;
pub mod load;
pub mod project;
pub mod refs;
pub mod scope;
pub mod segments;
pub mod taps;
pub mod types;

pub use chain_file::{ChainFile, ChainInput, EndStep, Item, Link, NodeItem, Start, Step, Subchain, decode_chain};
pub use derive::{CHAIN_IN, CHAIN_OUT, derive_chains, is_parked, lane_names};
pub use graph_file::decode_graph;
pub use layout::{Layout, decode_layout, read_layout};
pub use levels::{Unplayed, apply_link_levels, unplayed_levels};
pub use load::{Routing, load_routing};
pub use project::{ChainNode, ChainProject, is_chain_project, lane_file, read_chain_project};
pub use refs::with_chain_module_refs;
pub use scope::{ChainUnit, chain_units, split_key, sub_key};
pub use segments::{Built, picture_lane_of, to_segments};
pub use taps::{RoutedTap, TapOut, tap_outs_for, tap_pairs};
pub use types::{Chain, Out, Position, Segments, TapFeed, UseStep};

/// What one call to `load_routing` may produce: each key and destination written, each walk visit, each `runs` segment.
pub const BUDGET: usize = 1_000_000;

/// The sum a chain may start at, and the render node.
pub const MIX_NODE: &str = "MIX";
pub const RENDER_NODE: &str = "RENDER";

/// The port separator of a ref.
pub const PORT_SEP: char = '#';

/// A ref split into node and port; only the LAST `#` is the port, and a ref with an empty half is a plain name.
pub fn split_port(r: &str) -> (&str, Option<&str>) {
    match r.rfind(PORT_SEP) {
        Some(cut) if cut > 0 && cut + 1 < r.len() => (&r[..cut], Some(&r[cut + 1..])),
        _ => (r, None),
    }
}

/// A node and its port back into one ref.
pub fn port_ref(node: &str, port: Option<&str>) -> String {
    port.filter(|p| !p.is_empty())
        .map_or_else(|| node.to_owned(), |p| format!("{node}{PORT_SEP}{p}"))
}

/// What one call to `load_routing` has spent against `BUDGET`.
#[derive(Debug, Default)]
pub struct Budget {
    used: usize,
}

impl Budget {
    /// Spends `n`; false once the budget is passed.
    pub fn charge(&mut self, n: usize) -> bool {
        self.used = self.used.saturating_add(n);
        self.used <= BUDGET
    }

    pub fn used(&self) -> usize {
        self.used
    }
}

/// Where the budget stopped: the file the walk was in.
#[derive(Debug, PartialEq, Eq)]
pub struct Capped {
    pub file: String,
}
