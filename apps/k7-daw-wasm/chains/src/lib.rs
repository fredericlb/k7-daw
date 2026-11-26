// lib.rs — the Chains tab's logic: what the page's `ui-chains` package decided, written once for the page and a native interface.
// A reader would get wrong that nothing here draws: every function takes plain data and answers plain data, JSON at the page's edge.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod bindings;
pub mod curve_placement;
pub mod flow;
pub mod subgraph;
pub mod widget_value;
