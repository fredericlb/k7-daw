// doc_support/mod.rs — what the document tests share: the TypeScript's expected answers for the pure helpers (arrange, category, ownMix) and the gate's run.
// A reader would get wrong that nothing is computed in the tables: a table is a list of cases the TypeScript answered, as fixed text.

#![allow(dead_code)]

mod cases;
mod counts;
mod gate;
mod pictures;
mod provenance;
mod ts_order;
mod ts_text;
mod walk;
mod world;
#[allow(unused_imports)]
pub use cases::*;
#[allow(unused_imports)]
pub use counts::*;
#[allow(unused_imports)]
pub use gate::*;
#[allow(unused_imports)]
pub use pictures::*;
#[allow(unused_imports)]
pub use provenance::*;
#[allow(unused_imports)]
pub use ts_order::*;
#[allow(unused_imports)]
pub use ts_text::*;
#[allow(unused_imports)]
pub use walk::*;
#[allow(unused_imports)]
pub use world::*;
