//! window.rs — the capture window of a native write: the registry's hold on the root, released where it was opened (2157).
//! What a reader gets wrong: a root no native capture holds has no window, so closing it is a no-op.

use k7_capture::Captures;

/// A capture window; closed on drop (a client that hung up) when its owner did not close it.
pub struct Window {
    root: String,
    label: String,
    held: bool,
    armed: bool,
}

impl Window {
    /// Opens the window of `root`: a registry call, no round trip anywhere.
    pub async fn open(root: &str, label: &str) -> Window {
        Window {
            root: root.to_string(),
            label: label.to_string(),
            held: Captures::global().hold(root),
            armed: true,
        }
    }

    /// Closes the window: the last one's release sweeps.
    pub async fn close(mut self) {
        self.armed = false;
        if self.held {
            Captures::global().release(&self.root, &self.label);
        }
    }

    /// Whether a native capture holds the root.
    #[cfg(test)]
    pub fn is_native(&self) -> bool {
        self.held
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.armed && self.held {
            Captures::global().release(&self.root, &self.label);
        }
    }
}
