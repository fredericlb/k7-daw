//! atomic.rs — a file replaced whole or not at all, as `packages/song/src/atomicWrite.ts` does it: a hidden temp
//! beside it under the same name pattern (so watchers skip it), synced, then renamed over. On Windows a rename
//! refused because the other backend is reading the file is retried, as there.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

/// How often a refused rename is tried again on Windows, doubling the pause from 10 ms each time.
const RETRIES: u32 = 8;

/// Write `text` as `dir/name`, creating `dir`; owner-only, since what the front keeps holds keys and tokens.
pub fn write_atomic(dir: &Path, name: &str, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut nonce = [0u8; 4];
    let _ = getrandom::fill(&mut nonce);
    let temp = dir.join(format!(
        ".{name}.{}.{:08x}.tmp",
        std::process::id(),
        u32::from_le_bytes(nonce)
    ));
    let mut file = private(&temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    rename_retrying(&temp, &dir.join(name)).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// A new file, mode 0600 where there are modes.
fn private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
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

/// EPERM, EBUSY or EACCES, as node names what a scanner or another reader holding the file causes.
fn transient(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ResourceBusy
    ) || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(test)]
mod tests {
    /// What the front keeps (the AI key, the hunt's secrets) is readable by its own user alone.
    #[cfg(unix)]
    #[test]
    fn a_kept_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("k7-front-atomic-{}", std::process::id()));
        super::write_atomic(&dir, "a.json", "{}").unwrap();
        let mode = std::fs::metadata(dir.join("a.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
