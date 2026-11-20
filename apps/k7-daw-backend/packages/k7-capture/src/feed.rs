//! feed.rs — the paths a workspace reported, gathered for a moment and posted as one change event (2125).
//! What a reader gets wrong: the generation posted is the capture's at the moment of the post, not a count of paths.

use std::time::{Duration, Instant};

use indexmap::IndexSet;

/// How long paths are gathered before they are posted.
pub const GATHER: Duration = Duration::from_millis(100);

pub struct Feed {
    paths: IndexSet<String>,
    due: Option<Instant>,
    gather: Duration,
}

impl Feed {
    pub fn new(gather: Duration) -> Feed {
        Feed {
            paths: IndexSet::new(),
            due: None,
            gather,
        }
    }

    /// Paths reported at `now`: the gathering starts with the first, and a path already waiting is not repeated.
    pub fn push(&mut self, paths: Vec<String>, now: Instant) {
        if paths.is_empty() {
            return;
        }
        self.paths.extend(paths);
        self.due.get_or_insert(now + self.gather);
    }

    /// When the gathered paths are to be posted, if any wait.
    pub fn due(&self) -> Option<Instant> {
        self.due
    }

    /// The paths gathered, in the order reported; the next gathering starts empty.
    pub fn take(&mut self) -> Vec<String> {
        self.due = None;
        std::mem::take(&mut self.paths).into_iter().collect()
    }
}
