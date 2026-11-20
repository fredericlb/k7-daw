//! revisions_walked.rs — the token of a walk another reader took (1731): a journal's image and its token come from one
//! `walk_project`, so a write between the two can never make `after` equal `before` over files that moved.
//! What a reader gets wrong: the summary is NOT kept as the memo, since its stamp would be read after the walk.

use super::*;

impl Revisions {
    /// The token of `dir` for a summary the caller walked: the inputs settle as `token` would settle them.
    pub fn walked(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        summary: Summary,
        unknowable: bool,
    ) -> String {
        let now = (self.now)();
        let dropped = self.touch(root, now);
        if let Some(w) = self.watch {
            for r in &dropped {
                w.unwatch(r);
            }
            if self.feed.generation(root).is_none() {
                w.watch(root);
            }
        }
        let outside = outside_newest(home);
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let r = roots.get_mut(root).expect("touched");
        let inputs = Inputs {
            dir: Some(dir.to_string()),
            mutations: r.mutations,
            summary: Some(summary),
            outside,
            unknowable,
        };
        let epoch = self.settle(r, inputs, None);
        spell(&self.boot, epoch, true)
    }
}
