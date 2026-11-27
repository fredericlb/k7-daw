// wire/ — the shapes the file service's routes answer with, one struct per wire object, from which the page's TypeScript types are generated.
// A reader would get wrong that the routes do not serialise these: most write their JSON text by hand, and `tests/all/wire.rs` reads that text back through them.
// The choices are in apps/k7-daw-backend/docs/ts-types.md.

mod library;
mod machine;
mod workspace;

pub use library::*;
pub use machine::*;
pub use workspace::*;
