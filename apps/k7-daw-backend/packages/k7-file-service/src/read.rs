//! read.rs — a file's text as `Workspace.read` answers it: the gate, the binary names before any stat, a folder
//! refused, the text whole. What a reader gets wrong: `path` in the answer is the caller's spelling, not the
//! resolved one, so the route owns it; and a read that would block or swell is declined, never attempted (1045).

use std::fs::File;
use std::io::Read;

use crate::confine::Disk;
use crate::filter::{code_of, is_binary};
use crate::paths::basename;
use crate::space::{Declined, PathError, Space};
use crate::tree::Limits;

/// What `/file` answers besides the caller's `path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileText {
    pub binary: bool,
    pub content: String,
}

/// Why a file is not read, as `entryFailure` words it (1044).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    NotFound {
        rel: String,
    },
    IsFolder {
        rel: String,
    },
    Path(PathError),
    /// A file call failed: `ENOENT` after a good stat is a 404, any other code a 400.
    Io {
        code: &'static str,
    },
    Declined(Declined),
}

/// `read(rel)`: the text at `rel` in the workspace, or a `ReadError` naming why not.
pub fn read_text(space: &Space<'_>, rel: &str, limits: &Limits) -> Result<FileText, ReadError> {
    read_text_gated(space, rel, limits, &|_| true)
}

/// A regular file opened without blocking and without a link at its leaf: the stat that said it was one may be stale.
pub fn open_regular(path: &str) -> Result<File, ReadError> {
    let io = |e: std::io::Error| ReadError::Io { code: code_of(&e) };
    // The leaf is resolved here, so a link swapped in after the gate is refused by `O_NOFOLLOW` below.
    let real = std::fs::canonicalize(path).map_err(io)?;
    let mut open = std::fs::OpenOptions::new();
    open.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = open.open(real).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(ReadError::Declined(Declined::NotRegular));
    }
    Ok(file)
}

/// `read_text`, asking `admit` once the size is known whether a file of that many bytes may be held; no is `Busy`.
pub fn read_text_gated(
    space: &Space<'_>,
    rel: &str,
    limits: &Limits,
    admit: &dyn Fn(u64) -> bool,
) -> Result<FileText, ReadError> {
    if rel.len() > limits.max_rel {
        return Err(ReadError::Declined(Declined::LongPath));
    }
    let abs = space.safe_path(rel, &Disk).map_err(|e| match e {
        PathError::Declined(d) => ReadError::Declined(d),
        e => ReadError::Path(e),
    })?;
    // A binary name answers before any stat, a missing `.wav` included.
    if is_binary(&basename(&abs)) {
        return Ok(FileText {
            binary: true,
            content: String::new(),
        });
    }
    let meta = match std::fs::metadata(&abs) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadError::NotFound { rel: rel.into() });
        }
        Err(e) => return Err(ReadError::Io { code: code_of(&e) }),
    };
    if meta.is_dir() {
        return Err(ReadError::IsFolder { rel: rel.into() });
    }
    if !meta.is_file() {
        return Err(ReadError::Declined(Declined::NotRegular));
    }
    if meta.len() > limits.max_text {
        return Err(ReadError::Declined(Declined::TooLarge));
    }
    if !admit(meta.len()) {
        return Err(ReadError::Declined(Declined::Busy));
    }
    let file = open_regular(&abs)?;
    let mut bytes = Vec::new();
    // The stat may be stale: the read itself stops one byte past the limit.
    file.take(limits.max_text.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| ReadError::Io { code: code_of(&e) })?;
    if bytes.len() as u64 > limits.max_text {
        return Err(ReadError::Declined(Declined::TooLarge));
    }
    let content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    };
    Ok(FileText {
        binary: false,
        content,
    })
}
