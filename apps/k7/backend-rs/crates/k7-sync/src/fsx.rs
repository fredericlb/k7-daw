//! fsx.rs — the file calls the room's log makes, as node makes them: `writeFileAtomic` (a hidden temp beside the
//! file, synced, renamed over, the rename retried on Windows), an append, a read that never fails on bad UTF-8.
//! What a reader gets wrong: the temp's name follows `atomicWrite.ts`, so every watcher already skips it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Tests only: an ordering sync is a full flush, to measure what a barrier saves.
static ORDER_FULL: AtomicBool = AtomicBool::new(false);

/// Tests only: `true` makes `write_batched`'s ordering sync a full flush on Apple.
#[doc(hidden)]
pub fn order_with_full_flush(on: bool) {
    ORDER_FULL.store(on, Ordering::SeqCst);
}

/// How often a refused rename is tried again on Windows, doubling the pause from 10 ms each time.
const RETRIES: u32 = 8;

/// Write `text` as `file` whole or not at all.
pub fn write_atomic(file: &Path, text: &str) -> std::io::Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let mut nonce = [0u8; 4];
    let _ = getrandom::fill(&mut nonce);
    let temp = dir.join(format!(
        ".{name}.{}.{:08x}.tmp",
        std::process::id(),
        u32::from_le_bytes(nonce)
    ));
    let mut f = std::fs::File::create(&temp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    drop(f);
    rename_retrying(&temp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Write `text` as `file` whole or not at all, flushing nothing but what keeps a replaced file whole (2131b).
/// A file that replaces another, or any file when `ordered`, has its bytes ordered before the rename; `make_durable` is the rest.
pub fn write_batched(file: &Path, text: &str, ordered: bool) -> std::io::Result<()> {
    let ordered = ordered || std::fs::symlink_metadata(file).is_ok();
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let mut nonce = [0u8; 4];
    let _ = getrandom::fill(&mut nonce);
    let temp = dir.join(format!(
        ".{name}.{}.{:08x}.tmp",
        std::process::id(),
        u32::from_le_bytes(nonce)
    ));
    let mut f = std::fs::File::create(&temp)?;
    f.write_all(text.as_bytes())?;
    if ordered {
        order_before_rename(&f)?;
    }
    drop(f);
    rename_retrying(&temp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Apple: `F_BARRIERFSYNC` keeps the bytes ahead of the rename without waiting for the platter; elsewhere a plain sync.
fn order_before_rename(f: &std::fs::File) -> std::io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        use std::os::fd::AsRawFd;
        if ORDER_FULL.load(Ordering::SeqCst) {
            return f.sync_all();
        }
        // SAFETY: the descriptor is open for the whole call, and F_BARRIERFSYNC takes no argument.
        match unsafe { libc::fcntl(f.as_raw_fd(), libc::F_BARRIERFSYNC) } {
            -1 => Err(std::io::Error::last_os_error()),
            _ => Ok(()),
        }
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        f.sync_all()
    }
}

/// Make every file durable: a sync of each, on `threads` threads, then on Apple one full flush of the drive's cache.
/// A file that is gone is skipped; the first other error is returned after every file was tried.
pub fn make_durable(files: &[PathBuf], threads: usize) -> std::io::Result<()> {
    let threads = threads.clamp(1, files.len().max(1));
    let first_error = std::sync::Mutex::new(None);
    let last = std::sync::Mutex::new(None);
    let (first_error, last) = (&first_error, &last);
    std::thread::scope(|scope| {
        for chunk in files.chunks(files.len().div_ceil(threads).max(1)) {
            scope.spawn(move || {
                for path in chunk {
                    match sync_file(path) {
                        Ok(Some(f)) => *last.lock().unwrap_or_else(|e| e.into_inner()) = Some(f),
                        Ok(None) => {}
                        Err(e) => {
                            first_error
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .get_or_insert(e);
                        }
                    }
                }
            });
        }
    });
    if let Some(f) = last.lock().unwrap_or_else(|e| e.into_inner()).take() {
        flush_drive_cache(&f)?;
    }
    match first_error.lock().unwrap_or_else(|e| e.into_inner()).take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Sync one file's data, and hand its descriptor back so one flush of the cache can follow.
fn sync_file(path: &Path) -> std::io::Result<Option<std::fs::File>> {
    // Windows flushes through a handle opened for writing; elsewhere a read handle syncs as well.
    let f = match std::fs::OpenOptions::new()
        .read(true)
        .write(cfg!(windows))
        .open(path)
    {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        other => other?,
    };
    #[cfg(target_vendor = "apple")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor is open for the whole call.
        if unsafe { libc::fsync(f.as_raw_fd()) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }
    #[cfg(not(target_vendor = "apple"))]
    f.sync_all()?;
    Ok(Some(f))
}

/// Apple: one `F_FULLFSYNC` flushes the drive's cache for everything synced before it; elsewhere the syncs were enough.
fn flush_drive_cache(f: &std::fs::File) -> std::io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        f.sync_all()
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        let _ = f;
        Ok(())
    }
}

/// A rename, retried while a Windows scanner or reader holds the target.
pub fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut i = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if cfg!(windows) && i < RETRIES && transient(&e) => {
                std::thread::sleep(Duration::from_millis(10 << i));
                i += 1;
            }
            other => return other,
        }
    }
}

fn transient(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ResourceBusy
    ) || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// `appendFileSync`.
pub fn append(file: &Path, text: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    f.write_all(text.as_bytes())
}

/// `readFileSync(file, "utf8")`: a bad byte read as U+FFFD.
pub fn read_text(file: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(file)?;
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}
