//! import.rs — a foreign project planned for the workspace: the file read (dropped bytes or a workspace path), translated by `k7-import`, named.
//! What a reader gets wrong: nothing is written here; the route lands the binaries and the texts, and takes the folder away if they fail.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use k7_import::{
    Bytes, IMPORTABLE, Input, MAX_IMPORT_BYTES, SampleRef, World, check_droppable, check_size,
    emit_translation, ext_of, folder_name, translate,
};

use crate::confine::Disk;
use crate::filter::skip;
use crate::paths::join;
use crate::space::Space;

/// Why an import is refused: the status, the sentence, and whether the name somebody typed is what is taken.
#[derive(Debug, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
    pub taken: bool,
}

fn refuse(status: u16, message: impl Into<String>) -> Refused {
    Refused {
        status,
        message: message.into(),
        taken: false,
    }
}

/// What the import will land: the project's folder, its texts and binaries by path from the workspace's root, and the log.
pub struct Plan {
    pub project: String,
    pub texts: Vec<(String, String)>,
    pub binaries: Vec<(String, Bytes)>,
    pub log: Vec<String>,
}

/// The names the workspace's shared `_modules` already holds, which an imported module must not shadow.
fn taken_modules(root: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for kind in ["audio", "instruments", "transformers"] {
        let Ok(read) = std::fs::read_dir(join(&[root, "_modules", kind])) else {
            continue;
        };
        names.extend(
            read.flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase()),
        );
    }
    names
}

/// A top-level name of the workspace in use: a project, a file, a hidden one.
fn occupied(root: &str, name: &str) -> bool {
    std::fs::symlink_metadata(join(&[root, name])).is_ok()
}

fn named(root: &str, file: &str, typed: Option<&str>) -> Result<String, Refused> {
    folder_name(file, typed, &|n| occupied(root, n)).map_err(|message| Refused {
        status: 409,
        message,
        taken: true,
    })
}

fn world(root: &str, plugins: Option<HashSet<String>>) -> World {
    World {
        taken_modules: taken_modules(root),
        plugins,
    }
}

/// A file dropped on the home page: the self-contained formats, its bytes in hand, the folder named by the person.
///
/// # Errors
/// A format a drop does not take, an empty or oversized file, a name taken, or a file that does not translate.
pub fn plan_dropped(
    root: &str,
    file: &str,
    bytes: &[u8],
    typed: Option<&str>,
    plugins: Option<HashSet<String>>,
) -> Result<Plan, Refused> {
    check_droppable(file).map_err(|m| refuse(400, m))?;
    check_size(file, bytes.len()).map_err(|m| refuse(413, m))?;
    if bytes.is_empty() {
        return Err(refuse(
            400,
            format!("cannot import \"{file}\": the file is empty"),
        ));
    }
    let project = named(root, file, typed)?;
    let input = Input {
        source: file,
        file,
        bytes,
        read_sample: None,
        inflate_limit: None,
    };
    let t = translate(&input).map_err(|m| refuse(422, m))?;
    let made = emit_translation(file, &project, &t, &world(root, plugins), 0);
    Ok(Plan {
        project,
        texts: made.written.texts,
        binaries: made.written.binaries,
        log: made.log,
    })
}

/// A sample reference as a workspace path below the set's folder; none when it climbs out of the workspace.
fn sample_candidate(als_rel: &str, relative: &str) -> Option<String> {
    let als = als_rel.replace('\\', "/");
    let dir = als.rsplit_once('/').map_or("", |(d, _)| d);
    let rel = relative.replace('\\', "/");
    if rel.starts_with('/') || rel.as_bytes().get(1) == Some(&b':') {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    for part in dir.split('/').chain(rel.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            p => out.push(p),
        }
    }
    Some(out.join("/"))
}

/// The bytes of a workspace file, none when it is not a regular file the person may read.
fn read_in(space: &Space<'_>, rel: &str) -> Option<Vec<u8>> {
    if rel.split('/').any(skip) {
        return None;
    }
    let abs = space.safe_path(rel, &Disk).ok()?;
    let meta = std::fs::metadata(&abs)
        .ok()
        .filter(std::fs::Metadata::is_file)?;
    (meta.len() <= MAX_IMPORT_BYTES as u64).then_some(())?;
    std::fs::read(abs).ok()
}

/// The samples a `.als` names relative to itself, fetched from the workspace: two passes, since a parse reads them in order.
fn read_samples(
    space: &Space<'_>,
    als_rel: &str,
    file: &str,
    bytes: &[u8],
) -> Result<HashMap<String, Bytes>, Refused> {
    let wanted = RefCell::new(Vec::<String>::new());
    let dummy: Bytes = Arc::new(vec![0]);
    let first = |r: &SampleRef| -> Option<Bytes> {
        let at = sample_candidate(als_rel, r.relative.as_deref()?)?;
        wanted.borrow_mut().push(at);
        Some(dummy.clone())
    };
    let input = Input {
        source: als_rel,
        file,
        bytes,
        read_sample: Some(&first),
        inflate_limit: None,
    };
    translate(&input).map_err(|m| refuse(422, m))?;
    let mut held = HashMap::new();
    let mut total = 0usize;
    for at in wanted.into_inner() {
        if held.contains_key(&at) {
            continue;
        }
        let Some(data) = read_in(space, &at) else {
            continue;
        };
        total += data.len();
        if total > MAX_IMPORT_BYTES {
            return Err(refuse(
                413,
                "the samples this set names weigh more than an import takes",
            ));
        }
        held.insert(at, Arc::new(data));
    }
    Ok(held)
}

/// A file already in the workspace, clicked in the tree: a `.als` reads the samples beside it, counted outside otherwise.
///
/// # Errors
/// A path no file answers, a format not importable, an oversized file, or one that does not translate.
pub fn plan_path(
    space: &Space<'_>,
    rel: &str,
    typed: Option<&str>,
    plugins: Option<HashSet<String>>,
) -> Result<Plan, Refused> {
    let file = rel.replace('\\', "/");
    let file = file.rsplit('/').next().unwrap_or(&file).to_owned();
    let ext = ext_of(&file);
    if !IMPORTABLE.contains(&ext.as_str()) {
        return Err(refuse(
            400,
            format!(
                "cannot import \"{rel}\": expected one of {}",
                IMPORTABLE.join(", ")
            ),
        ));
    }
    let abs = space
        .safe_path(rel, &Disk)
        .map_err(|e| refuse(403, e.to_string()))?;
    let meta = std::fs::metadata(&abs)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .ok_or_else(|| refuse(404, format!("no such file: {rel}")))?;
    let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
    check_size(&file, size).map_err(|m| refuse(413, m))?;
    let bytes = std::fs::read(&abs).map_err(|_| refuse(404, format!("no such file: {rel}")))?;
    let root = space.root();
    let project = named(root, &file, typed)?;
    let samples = if ext == ".als" {
        read_samples(space, rel, &file, &bytes)?
    } else {
        HashMap::new()
    };
    let outside_set = RefCell::new(HashSet::<String>::new());
    let read = |r: &SampleRef| -> Option<Bytes> {
        let Some(relative) = r.relative.as_deref() else {
            // Counted by FILE, not by reference: nine clips off five files is five things to go and collect.
            outside_set
                .borrow_mut()
                .insert(r.absolute.clone().unwrap_or_else(|| r.name.clone()));
            return None;
        };
        match sample_candidate(rel, relative) {
            Some(at) => samples.get(&at).cloned(),
            None => {
                outside_set.borrow_mut().insert(relative.to_owned());
                None
            }
        }
    };
    let input = Input {
        source: rel,
        file: &file,
        bytes: &bytes,
        read_sample: (ext == ".als").then_some(&read),
        inflate_limit: None,
    };
    let t = translate(&input).map_err(|m| refuse(422, m))?;
    let made = emit_translation(
        rel,
        &project,
        &t,
        &world(root, plugins),
        outside_set.borrow().len(),
    );
    Ok(Plan {
        project,
        texts: made.written.texts,
        binaries: made.written.binaries,
        log: made.log,
    })
}

#[cfg(test)]
mod tests {
    use super::sample_candidate;

    #[test]
    fn a_sample_path_stays_below_the_workspace() {
        assert_eq!(
            sample_candidate("songs/a.als", "Samples/x.wav").as_deref(),
            Some("songs/Samples/x.wav")
        );
        assert_eq!(sample_candidate("a.als", "../x.wav"), None);
        assert_eq!(
            sample_candidate("songs/a.als", "..\\x.wav").as_deref(),
            Some("x.wav")
        );
        assert_eq!(sample_candidate("songs/a.als", "/etc/passwd"), None);
        assert_eq!(sample_candidate("songs/a.als", "C:\\x.wav"), None);
    }
}
