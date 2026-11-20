// source/disk.rs — the Source over the machine's files, the only file in the crate that names std::fs.
// A reader would get wrong that confinement is checked per directory: one real-path call each, kept for the instance's life.
// A link as the leaf is checked too, since a file link out of the mounts would otherwise read through.

use super::path::{Mounts, is_within, join, normalize, parent, windows_hazard};
use super::{Entry, Kind, Source, SourceError, Span, Stamp, Stat, sort_entries};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// An mtime this much older than now cannot be reproduced by a write in the same tick.
const SETTLE: Duration = Duration::from_secs(2);

pub struct DiskSource {
    mounts: Mounts,
    real: Vec<String>,
    real_files: Vec<String>,
    verdicts: Mutex<HashMap<String, bool>>,
}

impl DiskSource {
    /// A source serving `mounts`; a mount that does not exist is allowed and confines like one that does.
    pub fn new<S: AsRef<str>>(mounts: &[S]) -> Result<DiskSource, SourceError> {
        DiskSource::with_files(mounts, &[] as &[&str])
    }

    /// A source serving folder mounts and file mounts; a file mount admits that path alone, as a regular file.
    pub fn with_files<S: AsRef<str>, F: AsRef<str>>(dirs: &[S], files: &[F]) -> Result<DiskSource, SourceError> {
        let mounts = Mounts::with_files(dirs, files)?;
        let real = mounts
            .roots()
            .iter()
            .map(|r| real_path(r).unwrap_or_else(|| r.clone()))
            .collect();
        // A file mount's real path is its folder's real path and its name: a link as the file itself leads elsewhere.
        let real_files = mounts
            .file_roots()
            .iter()
            .map(|f| {
                let spelled = parent(f)
                    .and_then(|p| real_path(&p))
                    .zip(f.rsplit('/').next())
                    .map(|(p, name)| join(&p, name));
                spelled.unwrap_or_else(|| f.clone())
            })
            .collect();
        Ok(DiskSource {
            mounts,
            real,
            real_files,
            verdicts: Mutex::new(HashMap::new()),
        })
    }

    /// Whether the real path of `at` lies inside some mount's real path, asked once per path.
    fn real_inside(&self, at: &str) -> bool {
        let mut memo = self.verdicts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(v) = memo.get(at) {
            return *v;
        }
        let verdict = real_path(at).is_some_and(|r| self.real.iter().any(|m| is_within(m, &r)));
        memo.insert(at.to_owned(), verdict);
        verdict
    }

    /// Ok when `path` may be touched: inside a mount lexically, and no link on its way leads out of every mount.
    fn confine(&self, path: &str) -> Result<(), SourceError> {
        let containing = self.mounts.containing(path);
        if containing.is_empty() {
            return Err(SourceError::Outside);
        }
        if self.mounts.is_file_mount(path) {
            return match fs::symlink_metadata(path) {
                Err(_) => Ok(()),
                Ok(m) if m.is_dir() => Err(SourceError::Outside),
                Ok(_) if real_path(path).is_some_and(|r| self.real_files.contains(&r)) => Ok(()),
                Ok(_) => Err(SourceError::Outside),
            };
        }
        let mut probe = path.to_owned();
        let meta = loop {
            match fs::symlink_metadata(&probe) {
                Ok(m) => break m,
                Err(_) if containing.contains(&probe.as_str()) => return Ok(()),
                Err(_) => match parent(&probe) {
                    Some(up) => probe = up,
                    None => return Ok(()),
                },
            }
        };
        let checked = if meta.is_dir() || meta.file_type().is_symlink() {
            probe
        } else {
            parent(&probe).unwrap_or(probe)
        };
        if self.real_inside(&checked) {
            Ok(())
        } else {
            Err(SourceError::Outside)
        }
    }

    fn prepare(&self, path: &str) -> Result<String, SourceError> {
        let path = normalize(path)?;
        // Windows reads `\` as a separator and `:` as a stream, either of which could climb out of a lexical check.
        if cfg!(windows) && windows_hazard(&path) {
            return Err(SourceError::Outside);
        }
        self.confine(&path)?;
        Ok(path)
    }
}

/// The real path in the Source's spelling: no verbatim prefix, `/` for `\` where the OS uses it.
fn real_path(path: &str) -> Option<String> {
    let real = fs::canonicalize(path).ok()?;
    let text = real.to_str()?;
    let text = match text.strip_prefix(r"\\?\UNC\") {
        Some(unc) => format!("//{unc}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(text).to_owned(),
    };
    let text = if cfg!(windows) { text.replace('\\', "/") } else { text };
    normalize(&text).ok()
}

fn io_error(e: &std::io::Error) -> SourceError {
    match e.kind() {
        std::io::ErrorKind::NotFound => SourceError::NotFound,
        _ => SourceError::Io(e.to_string()),
    }
}

/// A folder or a regular file; anything else is not listed.
fn special_kind(meta: &fs::Metadata) -> Option<Kind> {
    if meta.is_dir() {
        Some(Kind::Dir)
    } else if meta.is_file() {
        Some(Kind::File)
    } else {
        None
    }
}

fn kind_of(meta: &fs::Metadata) -> Kind {
    if meta.is_dir() { Kind::Dir } else { Kind::File }
}

fn nanos(t: SystemTime) -> Option<u64> {
    u64::try_from(t.duration_since(UNIX_EPOCH).ok()?.as_nanos()).ok()
}

fn stamp_of(meta: &fs::Metadata) -> Stamp {
    let modified = meta.modified().ok();
    let settled = modified.is_some_and(|m| SystemTime::now().duration_since(m).is_ok_and(|age| age > SETTLE));
    Stamp {
        mtime_ns: modified.and_then(nanos).unwrap_or(0),
        len: meta.len(),
        settled,
    }
}

impl Source for DiskSource {
    fn stamps_are_times(&self) -> bool {
        true
    }

    fn folder_id(&self, path: &str) -> Option<String> {
        real_path(&self.prepare(path).ok()?)
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let path = self.prepare(path)?;
        let meta = fs::metadata(&path).map_err(|e| io_error(&e))?;
        Ok(Stat {
            kind: kind_of(&meta),
            len: meta.len(),
            stamp: stamp_of(&meta),
        })
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let dir = self.prepare(dir)?;
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| io_error(&e))? {
            let entry = entry.map_err(|e| io_error(&e))?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(kind) = entry.file_type() else { continue };
            let kind = if kind.is_symlink() {
                // A link counts as what it points at; a dangling one is not listed.
                let Ok(meta) = fs::metadata(join(&dir, &name)) else {
                    continue;
                };
                match special_kind(&meta) {
                    Some(k) => k,
                    None => continue,
                }
            } else if kind.is_dir() {
                Kind::Dir
            } else if kind.is_file() {
                Kind::File
            } else {
                // A socket, a pipe or a device is no document.
                continue;
            };
            out.push(Entry { name, kind });
        }
        sort_entries(&mut out);
        Ok(out)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let path = self.prepare(path)?;
        let mut file = fs::File::open(&path).map_err(|e| io_error(&e))?;
        let mut bytes = Vec::new();
        match span {
            Span::All => {
                file.read_to_end(&mut bytes).map_err(|e| io_error(&e))?;
            }
            Span::Range { offset, len } => {
                file.seek(SeekFrom::Start(offset)).map_err(|e| io_error(&e))?;
                file.take(len).read_to_end(&mut bytes).map_err(|e| io_error(&e))?;
            }
        }
        Ok(bytes)
    }
}
