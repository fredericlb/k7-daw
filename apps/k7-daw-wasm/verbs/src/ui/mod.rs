// ui/mod.rs — the command bar's, the menus' and the edits' rules, pure functions over JSON-shaped inputs the page hands in.
// A reader would get wrong that these read no file: the catalogue, the prefs and the lines arrive as arguments (docs/verbs/ui.md).

pub mod export;
pub mod fill;
pub mod fuzzy;
pub mod lines;
pub mod node_insert;
pub mod order;
pub mod pending;
pub mod prefs;
pub mod rank;
pub mod reload;
pub mod step;
pub mod writes;
