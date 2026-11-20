//! source.rs — where a worker hears its folder change: the process's watcher in production, a stand-in in tests.
//! What a reader gets wrong: `up` is asked, never remembered; a watcher that went down is pinned again at the next sweep.

use k7_file_service::watch::{Sink, Watch};

/// A pinned root's source of changed paths (2126).
pub trait Source: Send + Sync + 'static {
    /// Watch `root` until `unpin`, calling `sink` with the paths below it that changed; true when it is up.
    fn pin(&self, root: &str, sink: Sink) -> bool;
    /// Only the pin made with this `sink` is dropped: a root pinned again by a newer worker stays.
    fn unpin(&self, root: &str, sink: &Sink);
    fn up(&self, root: &str) -> bool;
}

impl Source for Watch {
    fn pin(&self, root: &str, sink: Sink) -> bool {
        Watch::pin(self, root, sink)
    }

    fn unpin(&self, root: &str, sink: &Sink) {
        Watch::unpin(self, root, sink);
    }

    fn up(&self, root: &str) -> bool {
        k7_file_service::watch::Feed::generation(self, root).is_some()
    }
}

/// The process's own watcher, shared with the revision memo.
pub struct Process;

impl Source for Process {
    fn pin(&self, root: &str, sink: Sink) -> bool {
        Source::pin(Watch::global(), root, sink)
    }

    fn unpin(&self, root: &str, sink: &Sink) {
        Source::unpin(Watch::global(), root, sink);
    }

    fn up(&self, root: &str) -> bool {
        Source::up(Watch::global(), root)
    }
}
