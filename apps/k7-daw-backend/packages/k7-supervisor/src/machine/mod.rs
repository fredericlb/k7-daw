//! machine — what the machine does when a server asks over a jobs channel, apart from the table that lists
//! the jobs (`parts/tunnel`): a slice of the merged log, an undo's audio kept a month, a render bounced
//! through the engine. What a reader gets wrong: nothing here touches the core or a path check; the job
//! modules check every path first (`docs/machine.md`).

pub mod history;
pub mod log_slice;
pub mod render;
