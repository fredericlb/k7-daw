//! parts — the supervisor's subsystems beside the core, each a task of its own fed `core::Event`s: the
//! registries, the engine queries, the tunnel, the jobs, the instruments, the slots… A milestone adds its
//! part as a module here and one line in `all()`; the order of that list is the order patches apply in.

use crate::core::PartSpec;

/// Every part a run starts, in order.
pub fn all() -> Vec<PartSpec> {
    vec![]
}
