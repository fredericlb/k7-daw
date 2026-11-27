//! bench/ensure.rs — `ensureBench`: the rig made fresh from a library chain on every open, unless it holds that chain with edits not saved.
//! What a reader gets wrong: a first open pins the workspace to the libraries' versions, and an unpinned bench declines.

use std::sync::atomic::{AtomicBool, Ordering};

use indexmap::IndexMap;
use song_edit::Fs;
use song_edit::scaffold_live::scaffold_live_graph;

use super::source::{bench_stamp, file, read_source, write_source};
use super::splice::splice_chain;
use super::{BenchError, Libs, PROJECT, WORKSPACE, bench_dir, with_chains, workspace_root};
use crate::edits::DiskFs;
use crate::listings::families::library_names;
use crate::paths::join;
use crate::space::Declined;

/// The address's chain: none for the bare rig, and `None` outside for an address that names no bench (`benchChainOf`).
#[must_use]
pub fn bench_chain_of(project: &str) -> Option<Option<String>> {
    song_schema::address::bench_chain_of(project)
}

/// Whether `r` is a bare `@<library>`, which names no chain.
fn is_bare_library(r: &str) -> bool {
    r.strip_prefix('@').is_some_and(|n| {
        !n.is_empty()
            && n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

/// What an open answers: the workspace and the project the rig is in.
#[must_use]
pub fn opened() -> serde_json::Value {
    serde_json::json!({ "workspace": WORKSPACE, "project": PROJECT })
}

fn refused<T>(e: impl ToString) -> Result<T, BenchError> {
    Err(BenchError::Refused(e.to_string()))
}

/// The bench `project` names: made fresh, unless it holds that chain already with edits not saved.
///
/// # Errors
/// `Declined` where the route is refused (no library, no pins), only before the first write, which sets `wrote`;
/// `Refused` with the sentence of a chain that cannot be played, or of one that could not be made after the rig was rewritten.
pub fn ensure_bench(libs: &Libs<'_>, project: &str, wrote: &AtomicBool) -> Result<(), BenchError> {
    let Some(chain) = bench_chain_of(project) else {
        return refused(format!("'{project}' is no bench"));
    };
    if let Some(c) = chain.as_deref().filter(|c| is_bare_library(c)) {
        return refused(format!("'{c}' is a library, not a chain"));
    }
    let home = libs.home;
    if library_names(home).is_empty() {
        return Err(Declined::NoLibraries.into());
    }
    let (root, dir) = (workspace_root(home), bench_dir(home));
    std::fs::create_dir_all(&root).or_else(refused)?;
    // The bench's workspace is pinned afresh once, as `pinBench` does before any view of it.
    crate::pins::ensure_fresh(home, &root)?;
    let mut fs = DiskFs::new(&[&root]).map_err(|e| BenchError::Refused(e.to_string()))?;
    // A reload must not cost the knobs turned since the bench opened: they are kept until saved or another chain is opened.
    if let Some(was) = read_source(&fs, &root)
        && was.ref_ == chain
        && fs.exists(&dir)
        && bench_stamp(&fs, &dir).is_ok_and(|now| now != was.stamp)
    {
        return Ok(());
    }
    // The record goes first: a crash past this point leaves a bench the next open makes fresh.
    let source = file(&root);
    fs.remove(&source).or_else(refused)?;
    wrote.store(true, Ordering::SeqCst);
    // Past the first write nothing is declined: a half-made rig would pass for the user's edit.
    let made = (|| -> Result<(), BenchError> {
        // A bench per CHAIN is an address that no longer exists; left there it would be a project saving nowhere.
        fs.remove_tree(&join(&[&root, "chains"])).or_else(refused)?;
        fs.remove_tree(&dir).or_else(refused)?;
        scaffold_live_graph(&mut fs, &dir, WORKSPACE)?;
        let ids = match &chain {
            None => IndexMap::new(),
            Some(c) => with_chains(libs, &dir, |chains, roots| {
                let (mounts, _) = roots.mounts().map_err(|_| Declined::WouldThrow)?;
                // The rig writes into the bench's own folder and reads the libraries.
                let mut rig =
                    DiskFs::reading(&[&dir], &mounts).map_err(|_| Declined::WouldThrow)?;
                Ok(splice_chain(&mut rig, &dir, chains, c)?.ids)
            })?,
        };
        write_source(&mut fs, &root, &dir, chain.as_deref(), &ids).or_else(refused)
    })();
    made.map_err(|e| match e {
        BenchError::Declined(d) => BenchError::Refused(format!(
            "the bench could not be made from its chain ({})",
            d.as_str()
        )),
        other => other,
    })
}
