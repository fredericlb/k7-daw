//! ignore.rs — k7's own ignore list: written to `.gitignore` when k7 makes a repository, and always to
//! `info/exclude` so a repository cloned from elsewhere keeps its own `.gitignore` and k7's caches stay out.
//! What a reader gets wrong: each name below is one that code writes or the brief names (docs/git.md).

use std::path::Path;

use crate::Result;

/// One ignored pattern and the code or decision that makes it.
pub const NAMES: &[(&str, &str)] = &[
    (
        ".k7-git.json",
        "the device's pointer to the repository, never shared",
    ),
    (".*.icloud", "Apple's placeholder for a file not downloaded"),
    (
        ".*.tmp",
        "the temp of an atomic write (k7-file-service atomic.rs `is_temp`)",
    ),
    (
        ".*.k7-part",
        "the partial file of a capture write or a landing (capture/write.rs, landing.rs)",
    ),
    (
        "*.part",
        "the ffmpeg prepare's partial file (verb_host/prepare.rs)",
    ),
    (
        "*.part-*",
        "a download's or a job's partial file (media/download.rs, jobs/render.rs, jobs/machine.rs)",
    ),
    (
        "*.cache",
        "the engine's waveform and analysis sidecars (workspaceFilter `skip`)",
    ),
    (
        ".renders/",
        "the automatic freezes' render cache (autoFreeze.md)",
    ),
    (
        "renders/",
        "a project's manual freezes (freeze.md), regenerable",
    ),
    ("stems/", "separated stems, regenerable (decision 4)"),
    (
        "exports/",
        "the picture cut and interchange files (picture.md)",
    ),
    ("_recycle_bin/", "the recycle bin (workspaceFilter `skip`)"),
    ("_history/", "the journal (workspaceFilter `skip`)"),
    ("node_modules/", "dependencies (workspaceFilter `skip`)"),
    (".DS_Store", "Finder writes one into any folder it opens"),
    ("Thumbs.db", "Explorer's thumbnail cache"),
];

const BEGIN: &str = "# k7 begin (rewritten by k7)";
const END: &str = "# k7 end";

/// The lines of the list, without the markers.
fn body() -> String {
    NAMES.iter().map(|(n, _)| format!("{n}\n")).collect()
}

/// The list between k7's markers, which an exclude file or a `.gitignore` carries.
pub fn block() -> String {
    format!("{BEGIN}\n{}{END}\n", body())
}

/// `existing` with k7's block replaced where it is, or appended.
pub fn with_block(existing: &str) -> String {
    let block = block();
    match (existing.find(BEGIN), existing.find(END)) {
        (Some(a), Some(b)) if a < b => {
            let tail = existing[b + END.len()..].trim_start_matches('\n');
            format!("{}{block}{tail}", &existing[..a])
        }
        _ if existing.is_empty() => block,
        _ if existing.ends_with('\n') => format!("{existing}{block}"),
        _ => format!("{existing}\n{block}"),
    }
}

/// `<gitdir>/info/exclude`, always carrying the list.
pub fn write_exclude(gitdir: &Path) -> Result<()> {
    let file = gitdir.join("info").join("exclude");
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let new = with_block(&old);
    if new != old {
        std::fs::create_dir_all(gitdir.join("info"))?;
        std::fs::write(&file, new)?;
    }
    Ok(())
}

/// `<workdir>/.gitignore` when there is none: k7 never edits a person's own.
pub fn write_gitignore(workdir: &Path) -> Result<()> {
    let file = workdir.join(".gitignore");
    if !file.exists() {
        std::fs::write(&file, block())?;
    }
    Ok(())
}
