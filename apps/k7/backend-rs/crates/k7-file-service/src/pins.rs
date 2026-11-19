//! pins.rs — `ensureLibraryPins` for a workspace pinned afresh: its `_library-pins.k7x`, written once with the newest
//! version of each library this machine holds.

use crate::atomic::write_atomic;
use crate::libraries::{Home, PINS_FILE};
use crate::listings::families::library_names;
use crate::listings::versions::newest_here;
use crate::paths::join;
use crate::space::Declined;
use crate::tree::Limits;

/// Whether a library name is a bare k7x key, which `stringify` writes without quotes.
fn bare(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The version a pins file names each library at when it is written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pinning {
    Newest,
    /// The open's pins: version 1, before versions existed.
    Opened,
    /// A made workspace's pins: the newest runnable version.
    Made,
}

/// The pins of `root` when it has none: written through a temp and a rename, answering whether this call wrote them.
pub fn ensure_fresh(home: &Home, root: &str) -> Result<bool, Declined> {
    ensure(home, root, Pinning::Newest)
}

/// `ensureLibraryPins(root, {fresh: true})`, which a made workspace runs.
pub fn ensure_made(home: &Home, root: &str) -> Result<bool, Declined> {
    ensure(home, root, Pinning::Made)
}

/// `ensureLibraryPins(root, {fresh: false})`, which `/open` runs: a workspace from before versions keeps version 1.
pub fn ensure_opened(home: &Home, root: &str) -> Result<bool, Declined> {
    ensure(home, root, Pinning::Opened)
}

fn ensure(home: &Home, root: &str, how: Pinning) -> Result<bool, Declined> {
    let file = join(&[root, PINS_FILE]);
    if std::fs::metadata(&file).is_ok() {
        return Ok(false);
    }
    let mut text = String::new();
    for name in library_names(home) {
        if !bare(&name) {
            return Err(Declined::WouldThrow);
        }
        let version = match how {
            Pinning::Opened => "1".to_string(),
            Pinning::Newest | Pinning::Made => {
                newest_here(home, &name, &Limits::default())?.to_string()
            }
        };
        text.push_str(&format!("{name}({version})\n"));
    }
    write_atomic(&file, text.as_bytes()).map_err(|_| Declined::WouldThrow)?;
    Ok(true)
}
