// lib.rs — the Chains tab's logic: what the page's `ui-chains` package decided, written once for the page and a native interface.
// A reader would get wrong that nothing here draws: every function takes plain data and answers plain data, JSON at the page's edge.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod assembly;
pub mod bindings;
pub mod build;
pub mod build_chains;
pub mod curve_placement;
pub mod flow;
pub mod geom;
pub mod graph;
pub mod graph_layout;
pub mod grid_fit;
pub mod module_row;
pub mod node;
pub mod node_ui;
pub mod panel;
pub mod row_seams;
pub mod stack_layout;
pub mod subgraph;
pub mod widget_value;

#[cfg(test)]
mod tests;
