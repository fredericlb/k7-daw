//! bench — the bench and the slot natively (3j O8, O9): one live rig in a workspace of its own, a library chain spliced in, a chain compiled into a slot.
//! What a reader gets wrong: `Declined` refuses the route (a library to pin); `Refused` is an answer, worded as the TypeScript words it.

mod ensure;
mod frozen;
mod slot;
mod source;
mod splice;

pub use ensure::{bench_chain_of, ensure_bench, opened};
pub use frozen::{Saved, copy_frozen_chain, delete_frozen_chain, save_bench_chain};
pub use slot::{SlotFile, slot_of};
pub use source::{BenchSource, read_source, write_source};
pub use splice::{Spliced, find_chain, splice_chain};

use std::collections::BTreeMap;
use std::sync::Arc;

use song::{DiskSource, Roots, Vocabulary};

use crate::chains::Chains;
use crate::confine::Disk;
use crate::libraries::Home;
use crate::paths::join;
use crate::song_ask::{Input, song_roots};
use crate::space::Declined;
use crate::tree::Limits;

/// The workspace the bench lives in and its one project (`BENCH_WORKSPACE`, `BENCH_PROJECT`).
pub const WORKSPACE: &str = song_schema::address::BENCH_WORKSPACE;
pub const PROJECT: &str = song_schema::address::BENCH_PROJECT;

/// The bench workspace's folder and the rig's folder in it.
#[must_use]
pub fn workspace_root(home: &Home) -> String {
    join(&[&home.studio, "workspace", WORKSPACE])
}

#[must_use]
pub fn bench_dir(home: &Home) -> String {
    join(&[&workspace_root(home), PROJECT])
}

/// Why an operation did not answer: the route is declined, or it is refused with the sentence the route sends.
#[derive(Debug)]
pub enum BenchError {
    Declined(Declined),
    Refused(String),
}

impl From<Declined> for BenchError {
    fn from(d: Declined) -> BenchError {
        BenchError::Declined(d)
    }
}

impl From<song_edit::Error> for BenchError {
    fn from(e: song_edit::Error) -> BenchError {
        BenchError::Refused(e.to_string())
    }
}

/// What the libraries are read through: the account's folders, and the vocabulary a caller's session holds or builds.
pub struct Libs<'a> {
    pub home: &'a Home,
    pub code_root: &'a str,
    pub env: &'a BTreeMap<String, String>,
    pub vocabulary: &'a dyn Fn(&Roots, &DiskSource) -> Option<Arc<Vocabulary>>,
}

/// The roots, the disk they mount and the chains of every library, as the project at `project` sees them.
pub(crate) fn with_chains<R>(
    libs: &Libs<'_>,
    project: &str,
    f: impl FnOnce(&Chains<'_>, &Roots) -> Result<R, BenchError>,
) -> Result<R, BenchError> {
    let input = Input {
        home: libs.home,
        project,
        code_root: libs.code_root,
        env: libs.env,
    };
    let roots = song_roots(&input, &Disk)?;
    let (dirs, files) = roots
        .mounts()
        .map_err(|_| BenchError::Declined(Declined::WouldThrow))?;
    let disk = DiskSource::with_files(&dirs, &files)
        .map_err(|_| BenchError::Declined(Declined::WouldThrow))?;
    let vocab =
        (libs.vocabulary)(&roots, &disk).ok_or(BenchError::Declined(Declined::WouldThrow))?;
    let chains = Chains::new(&disk, &roots, &vocab, &Limits::default())?;
    f(&chains, &roots)
}
