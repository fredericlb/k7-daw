// lib.rs — what the DJ desk (k7-plugins' turntables) decides, as Rust: the deck document, the click, the grid, loops, the controls.
// A reader would get wrong that nothing here draws or talks to the engine: the views and the sockets stay TypeScript (docs/turntables/).

// A NaN reads as "not positive" here, as the page's `!(x > 0)` does: the negated comparison is the rule.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod beats;
pub mod bindings;
pub mod controls;
pub mod deck;
pub mod deck_doc;
pub mod grid;
pub mod loops;
pub mod metronome;
pub mod rebuild;
pub mod set_list;
pub mod vocab;

/// `Math.round`: halves go toward +infinity, where `f64::round` sends them away from zero.
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}
