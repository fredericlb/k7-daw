// files.rs — what a status and a log answer, and what a Save takes of the status: the files but the ones too large for a host.
// A reader would get wrong that a too-large file is left out of the commit and stays in the workspace: it is not a change waiting.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "FileChange"))]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Modified,
    Removed,
    Renamed,
}

/// One changed path of the work tree against `HEAD`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "StatusEntry"))]
pub struct Entry {
    pub path: String,
    pub change: Change,
    /// The old path of a rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
    #[serde(default)]
    pub binary: bool,
    /// The file goes through LFS: git holds a pointer, the store the bytes.
    #[serde(default)]
    pub lfs: bool,
    /// Over the size a git host takes: a commit leaves it out and `status` keeps naming it.
    #[serde(default, rename = "tooLarge")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub too_large: bool,
}

/// One commit of the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "CommitInfo"))]
pub struct LogEntry {
    pub id: String,
    pub subject: String,
    pub author: String,
    /// Seconds since the epoch.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time: i64,
    /// The `K7-Device:` trailer, when the commit has one.
    pub device: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub files_changed: usize,
}

/// What a Save takes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CommitView {
    /// The changes the commit takes.
    pub sent: Vec<Entry>,
    /// The files over the host's limit: sent as the commit's exclusion and named in a toast.
    pub left_out: Vec<Entry>,
    /// How many of the sent go through LFS.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub lfs: usize,
}

#[must_use]
pub fn commit_view(entries: &[Entry]) -> CommitView {
    let (left_out, sent): (Vec<Entry>, Vec<Entry>) = entries.iter().cloned().partition(|e| e.too_large);
    let lfs = sent.iter().filter(|e| e.lfs).count();
    CommitView { sent, left_out, lfs }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, f: impl FnOnce(&mut Entry)) -> Entry {
        let mut e = Entry {
            path: path.into(),
            change: Change::Modified,
            from: None,
            size: 10,
            binary: false,
            lfs: false,
            too_large: false,
        };
        f(&mut e);
        e
    }
    fn paths(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn takes_nothing_when_nothing_changed() {
        assert!(commit_view(&[]).sent.is_empty());
    }

    #[test]
    fn leaves_a_too_large_file_out_and_names_it() {
        let big = row("big.wav", |e| {
            e.size = 3_000_000_000;
            e.too_large = true;
        });
        let v = commit_view(&[row("a.k7x", |_| {}), big]);
        assert_eq!(paths(&v.sent), ["a.k7x"]);
        assert_eq!(paths(&v.left_out), ["big.wav"]);
    }

    #[test]
    fn has_nothing_to_take_when_only_too_large_files_remain() {
        let big = row("big.wav", |e| e.too_large = true);
        assert!(commit_view(&[big]).sent.is_empty());
    }

    #[test]
    fn counts_the_files_that_go_through_lfs() {
        let lfs = |p| row(p, |e| e.lfs = true);
        assert_eq!(commit_view(&[lfs("a"), row("b", |_| {}), lfs("c")]).lfs, 2);
    }

    #[test]
    fn reads_a_fixture_that_leaves_the_flags_out() {
        let e: Entry = serde_json::from_str(r#"{"path":"a","change":"added","size":1}"#).unwrap();
        assert!(!e.too_large && !e.lfs && !e.binary);
    }
}
