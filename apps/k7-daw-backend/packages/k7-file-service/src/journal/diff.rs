//! diff.rs — what changed between two images: the text files that differ, the binaries created or overwritten (compared
//! as streams, never held), the files said as appeared, edited or gone, and the rebase of a record still running (1737).
//! What a reader gets wrong: a binary is judged by its pre-image, handed in by `snapshot`, not by its stamp.

use std::io::Read;

use serde_json::{Value, json};

use super::format::Change;
use super::image::{FileStamp, OMap};

/// A binary change: the staged copies of its two sides, `None` for a side the file lacked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinChange {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Text files that differ between two images, a missing side `None`, in the order JavaScript's `Set` of both key lists gives.
pub fn diff_text(before: &OMap<String>, after: &OMap<String>) -> Vec<Change> {
    let mut out = Vec::new();
    for path in before
        .keys()
        .chain(after.keys().filter(|k| !before.contains(k)))
    {
        let (b, a) = (before.get(path), after.get(path));
        if b != a {
            out.push(Change::new(path, b.cloned(), a.cloned()));
        }
    }
    out
}

/// Whether two files hold the same bytes, read in turn by blocks.
pub fn same_bytes(a: &str, b: &str) -> bool {
    let (Ok(fa), Ok(fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    if fa.metadata().map(|m| m.len()).ok() != fb.metadata().map(|m| m.len()).ok() {
        return false;
    }
    let (mut ra, mut rb) = (std::io::BufReader::new(fa), std::io::BufReader::new(fb));
    let (mut x, mut y) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let n = match ra.read(&mut x) {
            Ok(n) => n,
            Err(_) => return false,
        };
        if n == 0 {
            return rb.read(&mut y).is_ok_and(|m| m == 0);
        }
        if rb.read_exact(&mut y[..n]).is_err() || x[..n] != y[..n] {
            return false;
        }
    }
}

/// `diffBin`: binaries created or overwritten, by presence and by the pre-image `snapshot` staged (docs/file-journal.md).
pub fn diff_bin(
    before: &OMap<FileStamp>,
    after: &OMap<FileStamp>,
    pre: &OMap<String>,
    stage_after: &mut dyn FnMut(&str) -> Option<String>,
    unchanged: &mut dyn FnMut(&str, &str),
) -> Vec<BinChange> {
    let mut out = Vec::new();
    let paths = before
        .keys()
        .chain(after.keys().filter(|k| !before.contains(k)))
        .chain(
            pre.keys()
                .filter(|k| !before.contains(k) && !after.contains(k)),
        );
    for path in paths {
        let staged = |c: &mut dyn FnMut(&str) -> Option<String>| {
            after.contains(path).then(|| c(path)).flatten()
        };
        if !before.contains(path) {
            // Created: the pre-image, if any, is not the file's before.
            if let Some(a) = staged(stage_after) {
                out.push(BinChange {
                    path: path.clone(),
                    before: None,
                    after: Some(a),
                });
            }
            continue;
        }
        // Existed but no pre-image: nothing to restore to.
        let Some(pre_file) = pre.get(path) else {
            continue;
        };
        match staged(stage_after) {
            None => out.push(BinChange {
                path: path.clone(),
                before: Some(pre_file.clone()),
                after: None,
            }),
            Some(a) if !same_bytes(pre_file, &a) => out.push(BinChange {
                path: path.clone(),
                before: Some(pre_file.clone()),
                after: Some(a),
            }),
            Some(a) => unchanged(pre_file, &a),
        }
    }
    out
}

/// `ChangedFile`: a path below the project, appeared, edited or gone.
pub fn changed_files(base: &str, shapes: &[(&str, bool, bool)]) -> Vec<Value> {
    let head = if base == "." || base.is_empty() {
        String::new()
    } else {
        format!("{base}/")
    };
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for (path, before_none, after_none) in shapes {
        if !head.is_empty() && !path.starts_with(&head) {
            continue;
        }
        let rel = path.strip_prefix(&head).unwrap_or(path).to_string();
        let kind = if *before_none {
            "added"
        } else if *after_none {
            "removed"
        } else {
            "edited"
        };
        match out.iter_mut().find(|(p, _)| *p == rel) {
            // Appearing or disappearing outranks being rewritten.
            Some(seen) if seen.1 == "edited" => *seen = (rel, kind),
            Some(_) => {}
            None => out.push((rel, kind)),
        }
    }
    out.into_iter()
        .map(|(path, kind)| json!({ "path": path, "kind": kind }))
        .collect()
}
