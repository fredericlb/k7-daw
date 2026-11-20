//! watcher.rs — the one place a folder watcher is made, and what it watches: the OS's own, or polling where the OS
//! has none that scales (iOS, whose kqueue holds a descriptor per file), and there a library's published versions,
//! which never change, are left out (`docs/library-versions-plan.md`, `apps/k7-daw-engine/k7-runtime/docs/ios.md`).
//! What a reader gets wrong: a polled watcher scans on a timer, so an event is up to `POLL` late and each one has a thread.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventHandler, RecursiveMode, Watcher};

/// A watcher whichever kind was made, held by the callers that unwatch through it.
pub type Boxed = Box<dyn Watcher + Send>;

/// How often a polled watcher scans: an editor's save is heard within it, and a phone is not kept busy by it.
pub const POLL: Duration = Duration::from_secs(2);

/// Whether this host's watcher costs something per file, so the roots watched are cut to what can change.
pub const fn scarce() -> bool {
    cfg!(target_os = "ios")
}

/// A watcher of the host's kind.
pub fn make<F: EventHandler>(handler: F) -> notify::Result<Boxed> {
    make_as(scarce(), handler)
}

/// A polled watcher when `poll`, the OS's own otherwise; the choice is a parameter so a test can make either.
pub fn make_as<F: EventHandler>(poll: bool, handler: F) -> notify::Result<Boxed> {
    if poll {
        let config = notify::Config::default()
            .with_poll_interval(POLL)
            .with_compare_contents(false);
        return Ok(Box::new(notify::PollWatcher::new(handler, config)?));
    }
    Ok(Box::new(notify::recommended_watcher(handler)?))
}

/// A published version of a library: a folder named by an integer, with its manifest.
fn is_version(dir: &Path) -> bool {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_digit())
        && dir.join("library.k7x").is_file()
}

/// Whether `path` is a published version or lies inside one: nothing there changes.
pub fn under_frozen(path: &Path) -> bool {
    path.ancestors().any(is_version)
}

/// What to watch for `root`: the whole of it, or on a scarce host a library's folder without its published versions.
pub fn plan(root: &Path, scarce: bool) -> Vec<(PathBuf, RecursiveMode)> {
    let whole = vec![(root.to_path_buf(), RecursiveMode::Recursive)];
    if !scarce {
        return whole;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return whole;
    };
    let (mut live, mut frozen) = (Vec::new(), false);
    for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        if is_version(&dir) {
            frozen = true;
        } else {
            live.push(dir);
        }
    }
    if !frozen {
        return whole;
    }
    // The folder itself, for a version or a draft that appears; each live folder whole.
    let mut out = vec![(root.to_path_buf(), RecursiveMode::NonRecursive)];
    out.extend(live.into_iter().map(|p| (p, RecursiveMode::Recursive)));
    out
}
