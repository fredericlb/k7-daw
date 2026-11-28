// facades/ — the tables the op wrappers are written from: one entry per wrapper, grouped by the page's modules.
// A reader would get wrong that a table lists every op: an op whose wrapper reshapes its answer is wrapped by hand.

mod editor;
mod files;
mod small;

use crate::facade::Facade;

/// Every generated module.
pub fn all() -> Vec<Facade> {
    [small::all(), files::all(), editor::all()].into_iter().flatten().collect()
}
