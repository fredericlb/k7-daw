// source/bundle.rs — the JSON a server makes from what a compile read, and the bounds a bundle must keep to be replayed.
// A reader would get wrong that `absent` is a fact the disk gave (NotFound), where a path the bundle lacks is Missing: ask for more.
// The bounds are on the page, `docs/rustCompilerHost.md` (753, 797); a bundle past one is refused whole, never trimmed.

use super::SourceError;
use super::path::depth;
use crate::roots::Roots;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The most bytes, text and names together, a bundle carries; a Recorder stops adding past it and a bundle past it is not read.
pub const MAX_BUNDLE: usize = 128 << 20;
/// The most paths a bundle names in `refused`, `failedStats`, `failedLists`, `digests` and `absent` together.
pub const MAX_ENTRIES: usize = 200_000;
/// The most mounts a bundle names; every call looks through them.
pub const MAX_MOUNTS: usize = 4_096;
/// The longest key of a bundle, in bytes: twice Linux's `PATH_MAX`, the longest path a platform admits by default.
pub const MAX_PATH_LEN: usize = 8_192;
/// The most names below the root a key of a bundle has.
pub const MAX_PATH_DEPTH: usize = 1_024;

/// The JSON a server makes from the read set of its last compile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceBundle {
    pub format: u32,
    pub mounts: Vec<String>,
    pub files: BTreeMap<String, BundleFile>,
    pub lists: BTreeMap<String, Vec<super::Entry>>,
    /// Paths that answered `Outside`: they, and everything below them unless a mount lies below, do so again.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub refused: BTreeSet<String>,
    /// A `stat` that answered `Io`, with its message.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub failed_stats: BTreeMap<String, String>,
    /// A `list` that answered `Io`, with its message.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub failed_lists: BTreeMap<String, String>,
    /// Paths that answered `NotFound` and that no held listing proves absent.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub absent: BTreeSet<String>,
    /// The SHA-256 of a file the bundle holds no text of (2h's `known_digest`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub digests: BTreeMap<String, String>,
    /// The jsdsp sources the server found stale (2h's `known_stale`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub jsdsp_stale: BTreeSet<String>,
    /// The lengths in seconds of the audio the web answer measured (2j's `Lengths`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lengths: BTreeMap<String, f64>,
    /// The `lengths` table is complete: every audio file the server holds was measured, so one it lacks has no length to ask the header for.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub measured: bool,
    /// What a replay must ask to get the server's answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<BundleRequest>,
    /// Why the bundle stopped growing, when it did: a bundle that says so is incomplete and is never replayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow: Option<String>,
}

/// What a replay asks: the roots and the rig the server compiled with, the VST mode (`scan` or `none`) and the thawed freezes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BundleRequest {
    pub roots: Roots,
    pub rig: serde_json::Value,
    pub vsts: String,
    pub thawed: Vec<String>,
}

/// A file as the bundle holds it: its text; or its length alone, with `binary` (not UTF-8) or `io` (the read failed).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub len: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io: Option<String>,
}

fn refuse(what: String) -> SourceError {
    SourceError::Io(what)
}

/// Whether a key keeps to the bounds of a path.
fn check_key(key: &str) -> Result<(), SourceError> {
    if key.len() > MAX_PATH_LEN || depth(key) > MAX_PATH_DEPTH {
        let shown: String = key.chars().take(60).collect();
        return Err(refuse(format!(
            "a bundle key past {MAX_PATH_DEPTH} names or {MAX_PATH_LEN} bytes: {shown}"
        )));
    }
    Ok(())
}

impl SourceBundle {
    /// The bytes the bundle weighs: every key and every value, as a Recorder weighs what it keeps.
    pub fn weight(&self) -> usize {
        let keys = |it: &mut dyn Iterator<Item = usize>| it.fold(0usize, usize::saturating_add);
        let files = keys(
            &mut self
                .files
                .iter()
                .map(|(k, f)| k.len() + f.text.as_ref().map_or(8, String::len) + f.io.as_ref().map_or(0, String::len)),
        );
        let lists = keys(
            &mut self
                .lists
                .iter()
                .map(|(k, es)| k.len() + es.iter().fold(0usize, |n, e| n.saturating_add(e.name.len() + 8))),
        );
        let strings = |m: &BTreeMap<String, String>| keys(&mut m.iter().map(|(k, v)| k.len() + v.len()));
        let sets = |s: &BTreeSet<String>| keys(&mut s.iter().map(String::len));
        [
            keys(&mut self.mounts.iter().map(String::len)),
            files,
            lists,
            sets(&self.refused),
            sets(&self.absent),
            sets(&self.jsdsp_stale),
            strings(&self.failed_stats),
            strings(&self.failed_lists),
            strings(&self.digests),
            keys(&mut self.lengths.keys().map(|k| k.len() + 8)),
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }

    /// Whether the bundle can be replayed: format 1, whole, and inside every bound, checked before anything is built from it.
    pub fn check(&self) -> Result<(), SourceError> {
        if self.format != 1 {
            return Err(refuse(format!("bundle format {} is not 1", self.format)));
        }
        if let Some(why) = &self.overflow {
            return Err(refuse(format!("the bundle was cut short and is not replayed: {why}")));
        }
        let named = self.refused.len()
            + self.failed_stats.len()
            + self.failed_lists.len()
            + self.digests.len()
            + self.absent.len();
        if named > MAX_ENTRIES {
            return Err(refuse(format!(
                "a bundle names more than {MAX_ENTRIES} refused, failed, absent or digested paths"
            )));
        }
        if self.mounts.len() > MAX_MOUNTS {
            return Err(refuse(format!("a bundle names more than {MAX_MOUNTS} mounts")));
        }
        let keys = self
            .mounts
            .iter()
            .chain(self.files.keys())
            .chain(self.lists.keys())
            .chain(&self.refused)
            .chain(&self.absent)
            .chain(&self.jsdsp_stale)
            .chain(self.failed_stats.keys())
            .chain(self.failed_lists.keys())
            .chain(self.digests.keys())
            .chain(self.lengths.keys());
        for key in keys {
            check_key(key)?;
        }
        if self.weight() > MAX_BUNDLE {
            return Err(refuse(format!("a bundle weighs more than {MAX_BUNDLE} bytes")));
        }
        Ok(())
    }
}
