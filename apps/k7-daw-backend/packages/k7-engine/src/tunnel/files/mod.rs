//! files.rs — the hub's half of moving files, and how it confines them: a path is served only when it is
//! plain, lexically under a root the channel was opened with, AND its real path stays inside that root (or,
//! for a `perChild` root, inside its top-level folder). A link in a cloned repository may point anywhere.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use tokio::io::AsyncReadExt;

use super::frames::{CHUNK_BYTES, RemoteStat, Root};

/// At most this many paths per stat request.
const MAX_STAT_PATHS: usize = 2000;
const WIN: bool = cfg!(windows);

/// A segment `.` or `..`.
fn is_dot(seg: &str) -> bool {
    seg == "." || seg == ".."
}

/// `win32.isAbsolute`: rooted by a separator, or a drive letter followed by one.
fn win_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    matches!(b.first(), Some(b'/' | b'\\'))
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'/' | b'\\'))
}

/// Absolute with no `.` or `..` segment; on POSIX also `posix.normalize(p) === p`.
pub fn is_plain(p: &str) -> bool {
    if WIN {
        return win_absolute(p) && !p.split(['\\', '/']).any(is_dot);
    }
    if !p.starts_with('/') {
        return false;
    }
    let segs: Vec<&str> = p[1..].split('/').collect();
    let last = segs.len() - 1;
    // A trailing slash survives normalize; an empty segment anywhere else does not.
    p == "/"
        || segs
            .iter()
            .enumerate()
            .all(|(i, s)| !is_dot(s) && (!s.is_empty() || i == last))
}

/// A root spelt the Windows way: a drive letter or a UNC share.
fn is_windows_path(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || p.starts_with("\\\\")
}

/// `isWithin` of `@studio/web-protocol/paths`: `root` itself or below it, the separator honoured.
pub fn is_within(root: &str, path: &str) -> bool {
    if root.is_empty() || path.is_empty() {
        return false;
    }
    let win = is_windows_path(root);
    let (r, p) = if win {
        (root.to_lowercase(), path.to_lowercase())
    } else {
        (root.to_string(), path.to_string())
    };
    if p == r {
        return true;
    }
    let seps: &[char] = if win { &['\\', '/'] } else { &['/'] };
    seps.iter().any(|s| {
        let head = if r.ends_with(*s) {
            r.clone()
        } else {
            format!("{r}{s}")
        };
        p.starts_with(&head)
    })
}

/// Windows opens a DEVICE for these whatever follows the first dot (`segments.ts`).
fn reserved_device_name(seg: &str) -> bool {
    let base = seg
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str()) {
        return true;
    }
    let Some(n) = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"))
    else {
        return false;
    };
    matches!(
        n,
        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "\u{b9}" | "\u{b2}" | "\u{b3}"
    )
}

/// `safeSegment`: one segment that names a file and nothing else on any engine's machine.
pub fn safe_segment(seg: &str, windows: bool) -> bool {
    if reserved_device_name(seg)
        || seg.is_empty()
        || is_dot(seg)
        || seg.chars().all(|c| c == '.' || c == ' ')
    {
        return false;
    }
    if seg
        .chars()
        .any(|c| (c as u32) < 0x20 || matches!(c, '\u{7f}' | '/' | '\\' | ':'))
    {
        return false;
    }
    !(windows && (seg.ends_with('.') || seg.ends_with(' ')))
}

/// The root `p` lies in, lexically, every segment below it a safe name; a relative or non-normal path has none.
pub fn root_of<'a>(roots: &'a [Root], p: &str) -> Option<&'a Root> {
    if !is_plain(p) {
        return None;
    }
    let root = roots.iter().find(|r| is_within(&r.remote, p))?;
    let rest = p.get(root.remote.len()..).unwrap_or("");
    let seps: &[char] = if WIN { &['\\', '/'] } else { &['/'] };
    // A device name or a trailing dot opens something else than the file on Windows, whoever asks.
    let safe = rest
        .split(seps)
        .filter(|s| !s.is_empty())
        .all(|s| safe_segment(s, WIN));
    safe.then_some(root)
}

/// `realpath`, spelt as node spells it: no `\\?\` prefix on Windows.
pub async fn realpath(p: &Path) -> std::io::Result<String> {
    let real = tokio::fs::canonicalize(p).await?;
    let s = real.to_string_lossy().into_owned();
    Ok(match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with("UNC\\") => format!(r"\{}", &rest[3..]),
        Some(rest) => rest.to_string(),
        None => s,
    })
}

/// `path.join(root, first segment of p under root)` for a `perChild` root, else the root.
fn base_of(root: &Root, p: &str) -> PathBuf {
    let remote = root.remote.as_str();
    if root.per_child != Some(true) || p == remote {
        return PathBuf::from(remote);
    }
    let rest = p.get(remote.len() + 1..).unwrap_or("");
    let seps: &[char] = if WIN { &['\\', '/'] } else { &['/'] };
    let first = rest.split(seps).next().unwrap_or("");
    Path::new(remote).join(first)
}

/// Whether the REAL path of `p`, or of its nearest existing ancestor, stays inside its root.
pub async fn still_inside(root: &Root, p: &str) -> bool {
    let Ok(real_root) = realpath(&base_of(root, p)).await else {
        return false;
    };
    let mut probe = PathBuf::from(p);
    loop {
        if let Ok(real) = realpath(&probe).await {
            return is_within(&real_root, &real);
        }
        match probe.parent() {
            Some(up) if up != probe && !up.as_os_str().is_empty() => probe = up.to_path_buf(),
            _ => return false,
        }
    }
}

/// `Stats.mtimeMs` as node computes it.
pub fn mtime_ms(m: &std::fs::Metadata) -> f64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| {
            d.as_secs() as f64 * 1000.0 + f64::from(d.subsec_nanos()) / 1e6
        })
}

pub async fn stat_paths(roots: &[Root], paths: &[String]) -> Vec<RemoteStat> {
    let mut out = Vec::new();
    for path in paths.iter().take(MAX_STAT_PATHS) {
        let missing = RemoteStat::Missing { path: path.clone() };
        let Some(root) = root_of(roots, path) else {
            out.push(missing);
            continue;
        };
        if !still_inside(root, path).await {
            out.push(missing);
            continue;
        }
        out.push(match tokio::fs::metadata(path).await {
            Ok(m) if m.is_dir() => RemoteStat::Dir { path: path.clone() },
            Ok(m) => RemoteStat::File {
                path: path.clone(),
                size: m.len(),
                mtime_ms: mtime_ms(&m),
            },
            Err(_) => missing,
        });
    }
    out
}

/// One file as chunks, `send` told the last one; refused before any chunk when it may not be read.
pub async fn read_file_chunks<F, Fut>(roots: &[Root], path: &str, mut send: F) -> Result<(), String>
where
    F: FnMut(Vec<u8>, bool) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let inside = match root_of(roots, path) {
        Some(root) => still_inside(root, path).await,
        None => false,
    };
    if !inside {
        return Err(format!("not shared with this channel: {path}"));
    }
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| e.to_string())?
        .len();
    if size == 0 {
        send(Vec::new(), true).await;
        return Ok(());
    }
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut sent = 0u64;
    loop {
        let mut buf = vec![0u8; CHUNK_BYTES];
        let mut n = 0;
        while n < CHUNK_BYTES {
            let got = file.read(&mut buf[n..]).await.map_err(|e| e.to_string())?;
            if got == 0 {
                break;
            }
            n += got;
        }
        if n == 0 {
            break;
        }
        buf.truncate(n);
        sent += n as u64;
        send(buf, sent >= size).await;
    }
    if sent < size {
        send(Vec::new(), true).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
