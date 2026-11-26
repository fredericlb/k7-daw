//! table.rs — the jobs this machine does when a server asks on a jobs channel: ONE LINE PER JOB, each a module
//! of its own that registers its handler. A later milestone adds its lines here and its files beside; nothing
//! of the tunnel changes. `sync` is no line: the agent answers it itself, since it is the path map's.

use super::separate;
use crate::core::Ctx;
use crate::tunnel::Jobs;

/// Every job, by request `type`.
pub fn jobs(cx: &Ctx) -> Jobs {
    let mut jobs = Jobs::default();
    separate::register(&mut jobs, cx);
    jobs
}
