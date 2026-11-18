//! gate.rs — what a host route may touch: a path read only under a root the person reads, a file written only through `.job-out`.
//! What a reader gets wrong: a virtual path is judged lexically AND through the disk's links, and a refusal never names a real folder.

use std::collections::BTreeMap;

use super::Error;
use crate::changes::plan::{JOB_OUT, job_id, one_segment};
use crate::confine::{Disk, spelled};
use crate::libraries::Home;
use crate::paths::join;
use crate::song_ask::{Input, song_roots};
use crate::verb_bundle::library_roots;
use crate::verb_paths::Virtual;

pub fn forbidden(shown: &str) -> Error {
    let shown: String = shown.chars().take(80).collect();
    Error::new(403, format!("'{shown}' is outside the folders you read"))
}

/// The virtual roots of the open project, as `/verb-bundle` shows them.
pub fn virtual_for(
    home: &Home,
    root: &str,
    project: &str,
    code_root: &str,
    env: &BTreeMap<String, String>,
) -> Result<Virtual, Error> {
    let input = Input {
        home,
        project,
        code_root,
        env,
    };
    let roots = song_roots(&input, &Disk)
        .map_err(|d| Error::new(503, format!("declined: {}", d.as_str())))?;
    let libs = library_roots(&roots);
    Ok(Virtual::new(root, &libs, code_root, &home.studio, &roots))
}

/// The real path a virtual one names, for a read: absolute, no `..`, under a root, and the path the disk resolved it to.
pub fn read_path(v: &Virtual, shown: &str) -> Result<String, Error> {
    let bad = shown.contains(['\0', '\\']) || shown.len() > 4096;
    if !shown.starts_with('/') || bad {
        return Err(forbidden(shown));
    }
    if shown.split('/').any(portable_unsafe) {
        return Err(forbidden(shown));
    }
    let real = v.real(shown).ok_or_else(|| forbidden(shown))?;
    within(v, &real, shown)
}

/// A segment Windows reads as something else: a stream (`:`), a trailing dot or space, a device name with any extension.
fn portable_unsafe(seg: &str) -> bool {
    if seg.is_empty() || seg == "." || seg == ".." {
        return false;
    }
    if seg.contains(':') || seg.ends_with(['.', ' ']) {
        return true;
    }
    let stem = seg.split('.').next().unwrap_or("").trim_end_matches(' ');
    let up = stem.to_uppercase();
    let numbered = |p: &str| {
        up.strip_prefix(p).is_some_and(|n| {
            n.chars().count() == 1 && n.chars().all(|c| c.is_ascii_digit() || "¹²³".contains(c))
        })
    };
    matches!(
        up.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered("COM")
        || numbered("LPT")
}

/// The canonical form of `real` when it is still under a root after the links; one the disk cannot resolve is refused.
/// A path not made yet (a folder a render will make) is judged by its nearest folder that exists, a dangling link never.
pub fn within(v: &Virtual, real: &str, shown: &str) -> Result<String, Error> {
    let reached = |c: &std::path::Path| spelled(c).is_some_and(|s| v.reaches(&s));
    match std::fs::canonicalize(real) {
        Ok(c) if reached(&c) => spelled(&c).ok_or_else(|| forbidden(shown)),
        Ok(_) => Err(forbidden(shown)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if std::fs::symlink_metadata(real).is_ok() {
                return Err(forbidden(shown));
            }
            let path = std::path::Path::new(real);
            for up in path.ancestors().skip(1) {
                let Ok(c) = std::fs::canonicalize(up) else {
                    continue;
                };
                let rest = path.strip_prefix(up).unwrap_or(path);
                return if reached(&c) {
                    spelled(&c.join(rest)).ok_or_else(|| forbidden(shown))
                } else {
                    Err(forbidden(shown))
                };
            }
            Err(forbidden(shown))
        }
        Err(_) => Err(forbidden(shown)),
    }
}

/// The folder a job writes its outputs to, made; a `.job-out` or job folder that is a link is refused (3d's rule).
pub fn out_dir(studio: &str, job: &str) -> Result<String, Error> {
    if !job_id(job) {
        return Err(Error::new(400, "that is no job id"));
    }
    let top = join(&[studio, JOB_OUT]);
    let dir = join(&[&top, job]);
    for d in [&top, &dir] {
        match std::fs::symlink_metadata(d) {
            Ok(m) if m.is_symlink() => return Err(Error::new(403, "the job's folder is a link")),
            Ok(m) if !m.is_dir() => return Err(Error::new(403, "the job's folder is a file")),
            Ok(_) => {}
            Err(_) => std::fs::create_dir(d)
                .map_err(|_| Error::new(500, "the job's folder cannot be made"))?,
        }
    }
    Ok(dir)
}

/// The path of one output below `out_dir`: a name of one segment, never `.`-led, never a path.
pub fn out_file(dir: &str, out: &str) -> Result<String, Error> {
    if !one_segment(out) {
        return Err(Error::new(400, format!("'{out}' is no output name")));
    }
    Ok(join(&[dir, out]))
}

/// Removes a job's folder, for a job that ended without its value: its files are nobody's.
pub fn remove_out(studio: &str, job: &str) {
    if job_id(job) {
        let top = join(&[studio, JOB_OUT]);
        let dir = join(&[&top, job]);
        // A folder that is a link is left: only what the job itself made is removed.
        if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
