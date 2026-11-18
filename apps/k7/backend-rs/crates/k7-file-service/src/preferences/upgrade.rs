//! upgrade.rs — the one-time upgrade of `preferences.yaml` to `preferences.k7x`, safe among processes (`preferencesUpgrade.ts`, `fileLock.ts`).
//! What a reader gets wrong: nothing deletes data — the YAML is renamed to a backup, and the k7x is taken back only if it is still the bytes this call wrote.

use std::io;
use std::time::{Duration, Instant};

use crate::atomic::{Fs, Real, temp_for};
use crate::paths::dirname;

/// What the YAML becomes when it is set aside.
pub const BACKUP_SUFFIX: &str = ".pre-k7x";
/// A lock older than this lost its holder.
const STALE: Duration = Duration::from_secs(10);
/// The longest a changing caller waits for a held lock.
const WAIT: Duration = Duration::from_millis(300);

/// A text file's state: its text, absent, or present and unreadable with the errno name.
pub enum Read {
    Text(String),
    Absent,
    Error(String),
}

pub fn read_text(file: &str) -> Read {
    match std::fs::read(file) {
        Ok(b) => Read::Text(String::from_utf8_lossy(&b).into_owned()),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => Read::Absent,
            _ => Read::Error(crate::filter::code_of(&e).to_owned()),
        },
    }
}

fn present(file: &str) -> bool {
    std::fs::symlink_metadata(file).is_ok()
}

fn fsync_dir(dir: &str) {
    if let Ok(f) = std::fs::File::open(dir) {
        let _ = f.sync_all();
    }
}

/// Creates `file` whole only if the name is free; false when taken.
pub fn create_exclusive(
    file: &str,
    text: &str,
    mode: Option<u32>,
    held_lock: bool,
) -> io::Result<bool> {
    let tmp = temp_for(file);
    Real.write_temp(&tmp, &mut text.as_bytes(), mode)?;
    let linked = match std::fs::hard_link(&tmp, file) {
        Ok(()) => {
            let _ = std::fs::remove_file(&tmp);
            return Ok(true);
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&tmp);
            return Ok(false);
        }
        Err(e) => e,
    };
    // No hard links here: a held lock publishes by rename, else by an exclusive open.
    if !matches!(
        crate::filter::code_of(&linked),
        "EPERM" | "ENOSYS" | "ENOTSUP"
    ) && linked.kind() != io::ErrorKind::Unsupported
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(linked);
    }
    if held_lock {
        let landed = if present(file) {
            Ok(false)
        } else {
            std::fs::rename(&tmp, file).map(|()| true)
        };
        if !matches!(landed, Ok(true)) {
            let _ = std::fs::remove_file(&tmp);
        }
        return landed;
    }
    let _ = std::fs::remove_file(&tmp);
    if present(file) {
        return Ok(false);
    }
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
    {
        Ok(mut f) => {
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

/// The lock is held by someone else past the wait, or none can be made here.
pub enum Locked {
    Held(Lock),
    Busy,
    Unavailable,
}

pub struct Lock {
    file: String,
    token: String,
}

impl Drop for Lock {
    fn drop(&mut self) {
        if matches!(read_text(&self.file), Read::Text(t) if t == self.token) {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_default()
}

fn token() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let rand = temp_for("x");
    format!("{}\t{}\t{now}\t{rand}\n", std::process::id(), host())
}

/// Whether the lock may be broken: its age is past the limit or dated far ahead; the text is the one judged.
fn breakable(lock: &str) -> Option<String> {
    let mtime = std::fs::metadata(lock).ok()?.modified().ok()?;
    let aged = match std::time::SystemTime::now().duration_since(mtime) {
        Ok(age) => age > STALE,
        Err(ahead) => ahead.duration() > STALE,
    };
    let text = match read_text(lock) {
        Read::Text(t) => t,
        _ => String::new(),
    };
    (aged || holder_gone(&text)).then_some(text)
}

/// Whether the process that wrote `text` is gone on this host; false when that cannot be told.
fn holder_gone(text: &str) -> bool {
    let mut parts = text.split('\t');
    let (Some(pid), Some(from)) = (parts.next(), parts.next()) else {
        return false;
    };
    #[cfg(unix)]
    {
        let Ok(n) = pid.parse::<i32>() else {
            return false;
        };
        if n <= 1 || from.is_empty() || from != host() {
            return false;
        }
        // SAFETY: signal 0 only asks whether the process exists.
        let alive = unsafe { libc::kill(n, 0) } == 0;
        !alive && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, from);
        false
    }
}

/// Takes the lock; with `wait` it waits at most 300 ms, else it answers busy at once.
pub fn take_lock(lock: &str, wait: bool) -> Locked {
    let tok = token();
    let start = Instant::now();
    let mut judgments = 0;
    loop {
        match create_exclusive(lock, &tok, None, false) {
            Ok(true) => {
                return Locked::Held(Lock {
                    file: lock.to_owned(),
                    token: tok,
                });
            }
            Ok(false) => {}
            Err(_) => return Locked::Unavailable,
        }
        if let Some(judged) = breakable(lock) {
            if matches!(read_text(lock), Read::Text(t) if t == judged) {
                let _ = std::fs::remove_file(lock);
            }
            judgments += 1;
            if judgments <= 3 {
                continue;
            }
        }
        if !wait || start.elapsed() > WAIT {
            return Locked::Busy;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What the upgrade needs of the preferences it moves.
pub struct Spec<'a, T> {
    pub legacy: &'a str,
    pub target: &'a str,
    pub from_legacy: &'a dyn Fn(&str) -> Option<T>,
    pub print: &'a dyn Fn(&T) -> Option<String>,
    pub read_back: &'a dyn Fn(&str) -> Option<T>,
    pub equal: &'a dyn Fn(&T, &T) -> bool,
    pub mode: Option<u32>,
    pub wait: bool,
}

/// `retry`: the files changed, read again; `busy`: another process holds the upgrade; else the YAML's values.
pub enum Upgraded<T> {
    Retry,
    Values(T, bool),
}

/// The first free `<yaml>.pre-k7x`, then `.1`, `.2`…
fn backup_name(legacy: &str) -> String {
    let first = format!("{legacy}{BACKUP_SUFFIX}");
    if !present(&first) {
        return first;
    }
    (1..)
        .map(|n| format!("{first}.{n}"))
        .find(|c| !present(c))
        .unwrap_or(first)
}

fn set_aside(legacy: &str, dir: &str) {
    let to = backup_name(legacy);
    let _ = crate::atomic::rename_retrying(
        &Real,
        legacy,
        &to,
        crate::filter::rename_retry(),
        &std::thread::sleep,
    );
    fsync_dir(dir);
}

/// Takes the k7x back only if it is still the bytes this call wrote; a save that landed since stays.
fn take_back(target: &str, ours: &str) {
    let aside = temp_for(target);
    if std::fs::rename(target, &aside).is_err() {
        return;
    }
    if matches!(read_text(&aside), Read::Text(t) if t == ours) {
        let _ = std::fs::remove_file(&aside);
        return;
    }
    // Someone else's save goes back unless a newer file took the name meanwhile.
    match std::fs::hard_link(&aside, target) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => {
            if !present(target) {
                let _ = std::fs::rename(&aside, target);
            }
        }
    }
    let _ = std::fs::remove_file(&aside);
}

/// `upgradeLegacy`.
pub fn upgrade_legacy<T>(spec: &Spec<'_, T>) -> Upgraded<T> {
    let lock = match take_lock(&format!("{}.lock", spec.target), spec.wait) {
        Locked::Held(l) => l,
        other => {
            // Read-only, or a lock held past the wait: the YAML still answers and a later pass upgrades it.
            let values = match read_text(spec.legacy) {
                Read::Text(t) => (spec.from_legacy)(&t),
                _ => None,
            };
            return match values {
                None => Upgraded::Retry,
                Some(v) => Upgraded::Values(v, matches!(other, Locked::Busy)),
            };
        }
    };
    let _held = lock;
    // Everything is read again under the lock: another process may have upgraded, and a user saved since.
    let now = read_text(spec.target);
    let fresh = matches!(now, Read::Absent);
    if !fresh && !matches!(&now, Read::Text(t) if t.is_empty()) {
        return Upgraded::Retry;
    }
    let Read::Text(y) = read_text(spec.legacy) else {
        return Upgraded::Retry;
    };
    let Some(values) = (spec.from_legacy)(&y) else {
        return Upgraded::Retry;
    };
    let Some(text) = (spec.print)(&values) else {
        return Upgraded::Retry;
    };
    let dir = dirname(spec.target);
    let wrote = if fresh {
        match create_exclusive(spec.target, &text, spec.mode, true) {
            Ok(true) => Ok(()),
            Ok(false) => return Upgraded::Retry,
            Err(e) => Err(e),
        }
    } else {
        write_over(spec.target, &text, spec.mode)
    };
    if wrote.is_err() {
        return Upgraded::Values(values, false);
    }
    fsync_dir(&dir);
    let Read::Text(back) = read_text(spec.target) else {
        return Upgraded::Values(values, false);
    };
    match (spec.read_back)(&back) {
        Some(same) if (spec.equal)(&same, &values) => {
            set_aside(spec.legacy, &dir);
            Upgraded::Values(values, false)
        }
        _ => {
            take_back(spec.target, &text);
            Upgraded::Values(values, false)
        }
    }
}

/// `writeFileAtomic(target, text, { mode })`: the mode of an existing file kept, else the given one.
fn write_over(file: &str, text: &str, mode: Option<u32>) -> io::Result<()> {
    let tmp = temp_for(file);
    let want = Real.mode_of(file).or(mode);
    let done = Real
        .write_temp(&tmp, &mut text.as_bytes(), want)
        .and_then(|()| {
            crate::atomic::rename_retrying(
                &Real,
                &tmp,
                file,
                crate::filter::rename_retry(),
                &std::thread::sleep,
            )
        });
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}
