//! apply.rs — the disk's part of a write, once planned: the folders made, the rename and its retry, the recycle
//! bin's name, the upload's temp. What a reader gets wrong: `rename_with_retry` sleeps its thread; an async caller
//! drives `rename_once` and `RETRY` itself, so a held handle costs no blocking thread (1230).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::time::Duration;

use super::EntryError;
use crate::filter::code_of;
use crate::paths::{basename, dirname, is_within, join};
use crate::rel::to_rel;
use crate::space::Space;

/// What a rename is asked of: the disk, or a test's fake.
pub trait Renamer {
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
}

/// `std::fs::rename`, which on every host replaces a file it finds, as the shim's `renameSync` does (1232).
pub struct Disk;

impl Renamer for Disk {
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}

/// The retries a rename gets, and the wait between them: a scanner or a lingering handle lets go within seconds.
pub const RETRY: (u32, Duration) = (25, Duration::from_millis(100));

/// Whether a code is one a rename waits out; `EPERM` is never produced by `code_of` and stays for the record.
pub fn is_transient(code: &str) -> bool {
    matches!(code, "EPERM" | "EBUSY" | "EACCES" | "ENOTEMPTY")
}

fn io_error(e: &io::Error, syscall: &'static str, path: &str) -> EntryError {
    EntryError::Io {
        code: code_of(e),
        syscall,
        path: path.to_string(),
    }
}

/// One rename attempt, a failure worded as the shim's `fail` words it.
pub fn rename_once(r: &dyn Renamer, from: &str, to: &str) -> Result<(), EntryError> {
    r.rename(from, to).map_err(|e| io_error(&e, "rename", from))
}

/// `renameWithRetry`, blocking: for a caller with no runtime to sleep on, and the tests.
pub fn rename_with_retry(
    r: &dyn Renamer,
    from: &str,
    to: &str,
    tries: u32,
    delay: Duration,
) -> Result<(), EntryError> {
    let mut attempt = 0;
    loop {
        match rename_once(r, from, to) {
            Err(EntryError::Io { code, .. }) if is_transient(code) && attempt < tries => {
                attempt += 1;
                std::thread::sleep(delay);
            }
            done => return done,
        }
    }
}

/// `mkdirSync(abs, { recursive: true })`.
pub fn make_folder(abs: &str) -> Result<(), EntryError> {
    // The shim's mkdir is this same call, so a file in a parent's way answers as the platform says (EEXIST on Windows).
    std::fs::create_dir_all(abs).map_err(|e| io_error(&e, "mkdir", abs))
}

/// The first free name in `bin` for `base` by `exists`: `<base>`, else `<base>-2`, `-3`... with no bound (1231).
pub fn recycle_target_in(bin: &str, base: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let mut dest = join(&[bin, base]);
    let mut n = 2u64;
    while exists(&dest) {
        dest = join(&[bin, &format!("{base}-{n}")]);
        n += 1;
    }
    dest
}

/// `recycle_target_in` over the disk.
pub fn recycle_target(bin: &str, base: &str) -> String {
    recycle_target_in(bin, base, &|p| std::fs::metadata(p).is_ok())
}

/// The hidden temp an upload streams into, beside its target: `.<base>.<pid>.<8 hex>.tmp`, which every `skip` hides (1235).
pub fn temp_for(abs: &str, pid: u32, rand: u32) -> String {
    join(&[
        &dirname(abs),
        &format!(".{}.{pid}.{rand:08x}.tmp", basename(abs)),
    ])
}

/// Eight hex digits no other upload in flight draws: std's own per-process random keys, so the crate takes no dependency.
fn draw() -> u32 {
    let mut h = RandomState::new().build_hasher();
    h.write_u64(std::process::id().into());
    h.finish() as u32
}

/// An upload's first half: the parents made, and the temp's path (the `space` is the workspace the target belongs to).
pub fn upload_begin(_space: &Space<'_>, abs: &str) -> Result<String, EntryError> {
    make_folder(&dirname(abs))?;
    Ok(temp_for(abs, std::process::id(), draw()))
}

/// …and its last: the temp renamed over a target still free; a target outside the workspace and the libraries is refused.
pub fn upload_end(space: &Space<'_>, abs: &str, tmp: &str) -> Result<(), EntryError> {
    let inside = is_within(space.root(), abs)
        || crate::libraries::library_of(space.home(), abs)
            .is_some_and(|l| super::guards::library_writable(&l));
    if !inside || dirname(tmp) != dirname(abs) {
        return Err(EntryError::Refused(format!(
            "not an upload of this workspace: {abs}"
        )));
    }
    // Checked again: another upload of the same name may have landed while this one streamed.
    if std::fs::metadata(abs).is_ok() {
        return Err(EntryError::Exists {
            rel: to_rel(space, abs),
        });
    }
    rename_once(&Disk, tmp, abs)
}
