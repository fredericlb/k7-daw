//! casefold.rs — refuses a landing path that the disk would read as a tracked file of another case.
//! What a reader gets wrong: the disk is probed, not the OS name, so a case-sensitive volume lets both files land.

use super::plan::files_of;
use crate::{Error, Repo, Result};

/// The tracked path `p` would land on when the volume reads `exists(p)` for a file it does not track under that name.
fn clash<'a>(tracked: &'a [String], p: &str, exists: impl Fn(&str) -> bool) -> Option<&'a str> {
    if tracked.iter().any(|t| t == p) || !exists(p) {
        return None;
    }
    let folded = p.to_lowercase();
    tracked
        .iter()
        .map(String::as_str)
        .find(|t| t.to_lowercase() == folded)
}

impl Repo {
    /// Fails when a path about to land is not tracked here, exists on disk, and folds equal to a tracked one.
    pub(super) fn refuse_case_clash<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a str>,
    ) -> Result<()> {
        let tracked: Vec<String> = files_of(self.head_tree().as_ref()).into_keys().collect();
        let exists = |p: &str| self.workdir.join(p).symlink_metadata().is_ok();
        for p in paths {
            if let Some(mine) = clash(&tracked, p, exists) {
                return Err(Error::CaseClash {
                    theirs: p.to_string(),
                    mine: mine.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::clash;

    #[test]
    fn a_name_that_differs_by_case_and_resolves_on_disk_clashes() {
        let tracked = vec!["s/song.k7x".to_string(), "a.txt".to_string()];
        assert_eq!(clash(&tracked, "s/Song.k7x", |_| true), Some("s/song.k7x"));
        assert_eq!(clash(&tracked, "s/Song.k7x", |_| false), None);
        assert_eq!(clash(&tracked, "s/song.k7x", |_| true), None);
        assert_eq!(clash(&tracked, "b.txt", |_| true), None);
    }
}
