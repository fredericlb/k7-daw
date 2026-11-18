//! rooms_out.rs — the server writes each account room's head into the account's workspace folder, once, so the
//! next time the folder is read.
//! What a reader gets wrong: nothing is deleted or replaced, the log and the blobs stay, and a folder file that
//! differs from the room keeps its content, the room's copy going beside it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_sync::bases::parse_blob_ref;
use k7_sync::json::decode_uri_component;
use k7_sync::path::RELAY_DIR;
use k7_sync::relay::load_room;
use k7_sync::settle::Native;

/// One workspace folder name: no separator, no dot at the start, not the bench.
fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 200
        && !name.contains(['/', '\\', ':', '\0'])
        && !k7_file_service::filter::skip(name)
        && !name.eq_ignore_ascii_case(k7_file_service::workspaces::BENCH)
}

/// A room's path names a file inside the workspace and nowhere else, and none the capture hides.
fn inside(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != ".." && !k7_file_service::filter::skip(s))
}

/// The marker beside a room's log: its content is the head already written.
const DONE: &str = "folder.done";
/// An account's blob store, as the relay kept it.
const BLOBS: &str = ".sync-blobs";
/// The account that holds the libraries' rooms, which are no workspace.
const LIBRARIES: &str = "_libraries";

/// What one pass did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub rooms: usize,
    pub files: usize,
    pub kept_both: usize,
    pub skipped: usize,
}

/// Whether `rel` under `root` runs through a link, which a write must not follow.
fn through_link(root: &Path, rel: &str) -> bool {
    let mut at = root.to_path_buf();
    for part in rel.split('/') {
        at.push(part);
        if std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

/// The room's copy of `rel` for the `n`th try: `a.txt` becomes `a (server room).txt`, then `a (server room 2).txt`.
fn beside(rel: &str, n: usize) -> String {
    let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
    let (stem, ext) = name
        .rsplit_once('.')
        .filter(|(s, _)| !s.is_empty())
        .map_or((name, ""), |(s, e)| (s, e));
    let tag = if n <= 1 {
        "server room".to_string()
    } else {
        format!("server room {n}")
    };
    let name = if ext.is_empty() {
        format!("{stem} ({tag})")
    } else {
        format!("{stem} ({tag}).{ext}")
    };
    if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    }
}

fn bytes_of(blobs: &Path, text: &str) -> Result<Vec<u8>, String> {
    let Some((hash, size)) = parse_blob_ref(text) else {
        return Ok(text.as_bytes().to_vec());
    };
    let file = blobs.join(&hash[..2]).join(hash);
    let got = std::fs::read(&file).map_err(|e| format!("blob {hash}: {e}"))?;
    if got.len() as f64 != size {
        return Err(format!(
            "blob {hash}: {} bytes, the ref says {size}",
            got.len()
        ));
    }
    Ok(got)
}

fn put(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    k7_file_service::atomic::write_atomic(&file.to_string_lossy(), bytes)
}

/// The path the room's bytes go to, or None when the folder already holds them: a free name, never a taken one.
fn place(folder: &Path, rel: &str, bytes: &[u8], report: &mut Report) -> Option<PathBuf> {
    let dest = folder.join(rel);
    match std::fs::symlink_metadata(&dest) {
        Err(_) => return Some(dest),
        Ok(m) if !m.is_file() => {
            report.skipped += 1;
            return None;
        }
        Ok(_) if std::fs::read(&dest).is_ok_and(|have| have == bytes) => return None,
        Ok(_) => {}
    }
    for n in 1.. {
        let at = folder.join(beside(rel, n));
        if std::fs::symlink_metadata(&at).is_err() {
            report.kept_both += 1;
            return Some(at);
        }
        if std::fs::read(&at).is_ok_and(|have| have == bytes) {
            return None;
        }
    }
    None
}

/// One room's head written into `folder`; Ok(false) when a blob or a write failed, so the room is tried again.
fn write_room(
    log: &Path,
    blobs: &Path,
    folder: &Path,
    report: &mut Report,
) -> Result<bool, String> {
    let room = load_room(&Native, log).map_err(|e| e.message().to_string())?;
    let head = k7_sync::room::num(room.head);
    if std::fs::read_to_string(log.join(DONE)).is_ok_and(|d| d == head) {
        return Ok(true);
    }
    let mut complete = true;
    for (rel, text) in room.files() {
        if !inside(rel) || through_link(folder, rel) {
            report.skipped += 1;
            continue;
        }
        let bytes = match bytes_of(blobs, text) {
            Ok(b) => b,
            Err(_) => {
                report.skipped += 1;
                complete = false;
                continue;
            }
        };
        let Some(target) = place(folder, rel, &bytes, report) else {
            continue;
        };
        put(&target, &bytes).map_err(|e| format!("{rel}: {e}"))?;
        report.files += 1;
    }
    if complete {
        std::fs::write(log.join(DONE), head).map_err(|e| e.to_string())?;
    }
    Ok(complete)
}

/// Every room of the account written into `<account>/workspace/<name>`, a room done already left alone.
pub fn write_account(account: &Path) -> Report {
    let mut report = Report::default();
    let Ok(rooms) = std::fs::read_dir(account.join(RELAY_DIR)) else {
        return report;
    };
    let blobs = account.join(BLOBS);
    for e in rooms.flatten() {
        let Some(name) = e.file_name().to_str().and_then(decode_uri_component) else {
            continue;
        };
        if !name_ok(&name) || !e.path().is_dir() {
            continue;
        }
        let folder = account.join("workspace").join(&name);
        let before = (report.files, report.kept_both);
        match write_room(&e.path(), &blobs, &folder, &mut report) {
            Ok(_) => {
                if (report.files, report.kept_both) != before {
                    report.rooms += 1;
                }
            }
            Err(why) => eprintln!(
                "rooms | {}: room '{name}' not written: {why}",
                account.display()
            ),
        }
    }
    report
}

/// Every account under `data_root`, one after the other.
pub fn write_all(data_root: &Path) -> Report {
    let mut total = Report::default();
    let Ok(accounts) = std::fs::read_dir(data_root) else {
        return total;
    };
    let mut dirs: Vec<PathBuf> = accounts.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        if dir.file_name().is_some_and(|n| n == LIBRARIES) {
            continue;
        }
        let r = write_account(&dir);
        total.rooms += r.rooms;
        total.files += r.files;
        total.kept_both += r.kept_both;
        total.skipped += r.skipped;
    }
    total
}

/// On a server, at start and in the background: the rooms a device edited after the account last ran reach their folders.
pub fn launch(
    data_root: PathBuf,
    log: Arc<dyn Fn(&str) + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let done = tokio::task::spawn_blocking(move || write_all(&data_root)).await;
        match done {
            Ok(r) => log(&format!(
                "rooms | {} room(s) written into their folders: {} file(s), {} kept beside a differing folder file, {} skipped",
                r.rooms, r.files, r.kept_both, r.skipped
            )),
            Err(e) => log(&format!("rooms | the pass did not run: {e}")),
        }
    })
}

#[cfg(test)]
mod tests;
