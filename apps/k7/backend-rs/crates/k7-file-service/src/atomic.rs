//! atomic.rs — `writeFileAtomic`: a hidden temp beside the file, the file's mode kept, fsync, rename over; the rename
//! retried on Windows only (1750). What a reader gets wrong: the disk is a trait so a test can fill it or hold a rename;
//! the retry sleeps its thread, so it runs on a blocking one.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read};
use std::time::Duration;

use crate::filter::{code_of, rename_retry};
use crate::paths::{basename, dirname, join};

/// Renames refused this many times are given up on, 10 ms doubling: 2.55 s in all.
const RETRIES: u32 = 8;

/// What a write asks of the disk; a test's fake fills it or refuses a rename.
pub trait Fs: Sync {
    /// The temp created with `mode` (Unix), `data` written whole and flushed to the disk.
    fn write_temp(&self, tmp: &str, data: &mut dyn Read, mode: Option<u32>) -> io::Result<()>;
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    fn remove(&self, path: &str);
    /// The mode bits of an existing file, `None` where there are none or no file.
    fn mode_of(&self, path: &str) -> Option<u32>;
}

/// The real disk.
pub struct Real;

impl Fs for Real {
    fn write_temp(&self, tmp: &str, data: &mut dyn Read, mode: Option<u32>) -> io::Result<()> {
        let mut open = std::fs::OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::OpenOptionsExt;
            open.mode(m);
        }
        let mut f = open.open(tmp)?;
        // The umask applies to a new file's mode, not to the one an existing file already had.
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(m))?;
        }
        #[cfg(not(unix))]
        let _ = mode;
        io::copy(data, &mut f)?;
        // `sync_all` is `F_FULLFSYNC` on macOS, as libuv's fsync is.
        f.sync_all()
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove(&self, path: &str) {
        let _ = std::fs::remove_file(path);
    }

    fn mode_of(&self, path: &str) -> Option<u32> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(path)
                .ok()
                .map(|m| m.permissions().mode() & 0o7777)
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            None
        }
    }
}

/// `TEMP_NAME`: `.<base>.<pid>.<8 hex>.tmp`, which every `skip` hides.
pub fn is_temp(name: &str) -> bool {
    let Some(stem) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else {
        return false;
    };
    let mut parts = stem.rsplitn(3, '.');
    let (Some(rand), Some(pid), Some(base)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    rand.len() == 8
        && rand
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && !pid.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && !base.is_empty()
}

/// The temp a write of `file` goes through: hidden, beside it, unique to this process and call.
pub fn temp_for(file: &str) -> String {
    let mut h = RandomState::new().build_hasher();
    h.write_u64(std::process::id().into());
    let rand = h.finish() as u32;
    join(&[
        &dirname(file),
        &format!(".{}.{}.{rand:08x}.tmp", basename(file), std::process::id()),
    ])
}

fn transient(e: &io::Error) -> bool {
    matches!(code_of(e), "EPERM" | "EBUSY" | "EACCES")
}

/// A rename that waits out a Windows scanner or indexer holding the target; `retry` is true there only.
pub fn rename_retrying(
    fs: &dyn Fs,
    from: &str,
    to: &str,
    retry: bool,
    sleep: &dyn Fn(Duration),
) -> io::Result<()> {
    for i in 0.. {
        match fs.rename(from, to) {
            Err(e) if retry && transient(&e) && i < RETRIES => {
                sleep(Duration::from_millis(10 << i))
            }
            done => return done,
        }
    }
    unreachable!()
}

/// `data` to `file`, whole or not at all, on the disk `fs`; the temp is gone after, landed or not.
pub fn write_atomic_on(
    fs: &dyn Fs,
    file: &str,
    data: &mut dyn Read,
    retry: bool,
    sleep: &dyn Fn(Duration),
) -> io::Result<()> {
    let tmp = temp_for(file);
    let mode = fs.mode_of(file);
    let done = fs
        .write_temp(&tmp, data, mode)
        .and_then(|()| rename_retrying(fs, &tmp, file, retry, sleep));
    if done.is_err() {
        fs.remove(&tmp);
    }
    done
}

/// `writeFileAtomic(file, bytes)` on the real disk.
pub fn write_atomic(file: &str, bytes: &[u8]) -> io::Result<()> {
    write_atomic_on(
        &Real,
        file,
        &mut &bytes[..],
        rename_retry(),
        &std::thread::sleep,
    )
}

/// The same from a reader, a blob's copy: never held in memory.
pub fn write_atomic_from(file: &str, data: &mut dyn Read) -> io::Result<()> {
    write_atomic_on(&Real, file, data, rename_retry(), &std::thread::sleep)
}
