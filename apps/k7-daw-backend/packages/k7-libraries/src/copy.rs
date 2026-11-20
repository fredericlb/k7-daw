//! copy.rs — a folder copied with several files in flight. `copyTree.ts`.
//! What a reader gets wrong: a link in the source is skipped, never followed (docs/libraries.md, "Departures").

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Files copied at once; past this the disk, not the loop, is what waits.
const IN_FLIGHT: usize = 8;

/// Copy `from` into `to` (created), skipping every path `keep` refuses, and the whole folder under a refused one.
pub fn copy_tree(from: &Path, to: &Path, keep: &dyn Fn(&Path) -> bool) -> io::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    visit(from, to, Path::new(""), keep, &mut files)?;
    let next = AtomicUsize::new(0);
    let failed: Mutex<Option<io::Error>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..IN_FLIGHT.min(files.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { return };
                    if let Err(e) = fs::copy(from.join(rel), to.join(rel)) {
                        failed
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .get_or_insert(e);
                        next.store(files.len(), Ordering::Relaxed);
                        return;
                    }
                }
            });
        }
    });
    match failed.into_inner().unwrap_or_else(|p| p.into_inner()) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn visit(
    from: &Path,
    to: &Path,
    rel: &Path,
    keep: &dyn Fn(&Path) -> bool,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    fs::create_dir_all(to.join(rel))?;
    let mut entries: Vec<_> = fs::read_dir(from.join(rel))?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let child = rel.join(e.file_name());
        if !keep(&from.join(&child)) {
            continue;
        }
        let kind = e.file_type()?;
        if kind.is_dir() {
            visit(from, to, &child, keep, files)?;
        } else if kind.is_file() {
            files.push(child);
        }
    }
    Ok(())
}
