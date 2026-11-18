//! disk.rs — where a workspace's capture state lives under `<home>/.sync/`: the log folder's name, the replica's id,
//! the head, the batch log, and the confined path a frame may name (`liveLog.ts`, `livePath.ts`, `liveRelay.ts`).
//! What a reader gets wrong: a read answers absent only for `NotFound`; any other error is "not now" (2102).

use std::io::{self, ErrorKind, Write};
use std::path::Path;

use k7_file_service::paths::{basename, is_within, join, resolve_all};
use k7_sync::fsx::write_atomic;
use k7_sync::json::stringify;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::num::js_number;

/// The batch log rotates once it is past this many bytes.
pub const LOG_CEILING: u64 = 20 * 1024 * 1024;

/// A drive-letter path spelled with backslashes, as node's win32 `resolve` gives it (`windowsSpelling`, history.ts).
pub fn windows_spelling(abs: &str) -> String {
    let b = abs.as_bytes();
    let drive =
        b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    if drive {
        abs.replace('/', "\\")
    } else {
        abs.to_string()
    }
}

/// `[^\w.-]+` replaced by `_`, a run of other characters becoming one underscore.
fn folder_name(base: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in base.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    out
}

/// The log folder of the workspace at `root`: its name for a reader, a hash of its path for uniqueness (`liveLogFor`).
pub fn log_dir_of(studio_home: &str, root: &str) -> String {
    let abs = resolve_all(&[root]);
    let name = folder_name(&basename(&abs));
    let name = if name.is_empty() { "workspace" } else { &name };
    let digest = Sha256::digest(windows_spelling(&abs).as_bytes());
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    join(&[studio_home, ".sync", &format!("{name}-{hex}")])
}

/// A UUID v4 in its canonical text, as `randomUUID` spells it.
pub fn uuid_v4() -> io::Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b)
        .map_err(|e| io::Error::other(format!("no randomness for an id: {e}")))?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}

/// This replica's id, made once and kept; an unreadable or empty file is an error, never a new id (2102, 2109).
pub fn replica_id(studio_home: &str) -> io::Result<String> {
    let dir = join(&[studio_home, ".sync"]);
    let file = join(&[&dir, "replica-id"]);
    match std::fs::read(&file) {
        Ok(b) => match String::from_utf8_lossy(&b).trim() {
            "" => Err(io::Error::new(
                ErrorKind::InvalidData,
                "replica-id: empty, left as it is",
            )),
            id => Ok(id.to_string()),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            std::fs::create_dir_all(&dir)?;
            let id = uuid_v4()?;
            write_atomic(Path::new(&file), &id)?;
            Ok(id)
        }
        Err(e) => Err(e),
    }
}

/// The head file's number as JavaScript reads it (negative is 0, fractional its whole part); no number is None (2109).
fn head_of(text: &str) -> Option<u64> {
    let n = js_number(text);
    if n.is_nan() {
        None
    } else {
        Some(if n < 1.0 { 0 } else { n as u64 })
    }
}

/// The last seq a workspace applied; absent is zero, an unreadable or unparseable file is an error naming it (2102, 2109).
pub fn load_head(log_dir: &str) -> io::Result<u64> {
    match std::fs::read(join(&[log_dir, "head"])) {
        Ok(b) => head_of(&String::from_utf8_lossy(&b)).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "head: not a number, left as it is")
        }),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

/// The head, written whole or not at all after each entry and welcome.
pub fn save_head(log_dir: &str, head: u64) -> io::Result<()> {
    std::fs::create_dir_all(log_dir)?;
    write_atomic(Path::new(&join(&[log_dir, "head"])), &head.to_string())
}

/// `rel` under `root` as an absolute path that cannot leave `root`; the error says why it is refused (`confinedPath`).
pub fn confined_path(root: &str, rel: &str) -> Result<String, String> {
    k7_sync::path::assert_sync_path(rel)?;
    let base = resolve_all(&[root]);
    let mut parts = vec![base.as_str()];
    parts.extend(rel.split('/'));
    let abs = resolve_all(&parts);
    // Strictly below: the root itself is no file of it.
    if !(is_within(&base, &abs) && !is_within(&abs, &base)) {
        let quoted = stringify(&Value::String(rel.into()));
        return Err(format!("refused sync path {quoted}: outside {base}"));
    }
    Ok(abs)
}

/// The batches a workspace captured, one JSON line each, rotated once at the ceiling.
pub struct LiveLog {
    dir: String,
}

impl LiveLog {
    pub fn new(dir: &str) -> Self {
        LiveLog {
            dir: dir.to_string(),
        }
    }

    pub fn dir(&self) -> &str {
        &self.dir
    }

    fn file(&self) -> String {
        join(&[&self.dir, "batches.jsonl"])
    }

    pub fn append(&self, batch: &Value) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let file = self.file();
        match std::fs::metadata(&file) {
            Ok(m) if m.len() > LOG_CEILING => {
                std::fs::rename(&file, join(&[&self.dir, "batches.1.jsonl"]))?
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)?;
        f.write_all(format!("{}\n", stringify(batch)).as_bytes())
    }

    /// The last `n` batches of the current log, oldest first; `n` is a JavaScript number, as `slice(-n)` takes it.
    pub fn tail(&self, n: f64) -> io::Result<Vec<Value>> {
        let text = match std::fs::read(self.file()) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let lines: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
        let len = lines.len() as f64;
        // `slice(-n)`: the start is `-n` cut toward zero, counted from the end when negative.
        let from = -n.trunc();
        let start = if from < 0.0 {
            (len + from).max(0.0)
        } else {
            from.min(len)
        };
        lines[start as usize..]
            .iter()
            .map(|l| serde_json::from_str(l).map_err(|e| io::Error::new(ErrorKind::InvalidData, e)))
            .collect()
    }
}
