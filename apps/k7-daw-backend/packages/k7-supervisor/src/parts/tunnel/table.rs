//! table.rs — the jobs this machine does when a server asks on a jobs channel: ONE LINE PER JOB, each a module
//! of its own that registers its handler. A later milestone adds its lines here and its files beside; nothing
//! of the tunnel changes. `sync` is no line: the agent answers it itself, since it is the path map's.

use std::sync::Arc;

use super::render::Rendering;
use super::{files, log, render, separate, settings, slot_jobs};
use crate::core::Ctx;
use crate::tunnel::Jobs;

/// Every job, by request `type`.
pub fn jobs(cx: &Ctx, rendering: &Arc<Rendering>) -> Jobs {
    let mut jobs = Jobs::default();
    separate::register(&mut jobs, cx);
    log::register(&mut jobs, cx);
    files::register(&mut jobs, cx);
    settings::register(&mut jobs, cx);
    slot_jobs::register(&mut jobs, cx);
    render::register(&mut jobs, cx, rendering);
    jobs
}
