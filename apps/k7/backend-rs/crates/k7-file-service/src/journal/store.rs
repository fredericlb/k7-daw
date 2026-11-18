//! store.rs — a project's journal folder: its name (`historyStoreFor`), the journal read whole with its stamp, persisted
//! atomically, a torn file set aside, and the leftovers of a crash swept (1722 to 1726). What a reader gets wrong: `load`
//! changes nothing on disk, so a reader can read a journal in place; the writer sets a torn file aside itself.

use std::io::Read;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use super::format::{Journal, Torn, parse, print};
use super::lossy::{Lossy, read_lossy};
use crate::atomic::{is_temp, write_atomic};
use crate::libraries::Home;
use crate::paths::{basename, join, resolve_all};

/// `HISTORY_HOME_DIR`.
pub const HISTORY_DIR: &str = ".history";
/// A temp of `journal.json` older than this is a crash's.
const TEMP_AGE: Duration = Duration::from_secs(3600);

/// `windowsSpelling`: a drive-letter path with backslashes, whichever runtime hashes it.
pub fn windows_spelling(abs: &str) -> String {
    let b = abs.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    {
        abs.replace('/', "\\")
    } else {
        abs.to_string()
    }
}

/// `<studio>/.history/<name>-<12 hex>`: the folder of the project at `dir`, spelled as node spelled it (1723).
pub fn store_for(home: &Home, dir: &str) -> String {
    let abs = resolve_all(&[dir]);
    let mut name = String::new();
    let mut in_run = false;
    for c in basename(&abs).chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            name.push(c);
            in_run = false;
        } else if !in_run {
            name.push('_');
            in_run = true;
        }
    }
    if name.is_empty() {
        name.push_str("project");
    }
    let digest = Sha256::digest(windows_spelling(&abs).as_bytes());
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    join(&[&home.studio, HISTORY_DIR, &format!("{name}-{hex}")])
}

/// What changes when a file is written: its length and modification time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub len: u64,
    pub mtime: Option<SystemTime>,
}

/// What a store holds.
#[derive(Debug)]
pub enum Loaded {
    /// No `journal.json`: a fresh journal.
    Absent,
    /// The journal, its stamp, and what reading it repaired (1796).
    Journal(Box<Journal>, Stamp, Lossy),
    /// A file that does not read (1754); `stamp` is its own.
    Torn(Torn, Option<Stamp>),
    /// A read that failed (EMFILE, EIO, EACCES): the file is healthy as far as anyone knows and stays where it is.
    Unreadable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub dir: String,
}

impl Store {
    pub fn new(dir: String) -> Store {
        Store { dir }
    }

    pub fn journal_path(&self) -> String {
        join(&[&self.dir, "journal.json"])
    }

    pub fn blobs_dir(&self) -> String {
        join(&[&self.dir, "blobs"])
    }

    pub fn staged_dir(&self) -> String {
        join(&[&self.dir, "staged"])
    }

    pub fn blob_path(&self, name: &str) -> String {
        join(&[&self.blobs_dir(), name])
    }

    /// The journal file's stamp, `None` when there is no file.
    pub fn stamp(&self) -> Option<Stamp> {
        let m = std::fs::metadata(self.journal_path()).ok()?;
        Some(Stamp {
            len: m.len(),
            mtime: m.modified().ok(),
        })
    }

    /// The journal read whole (1722); nothing on disk changes.
    pub fn load(&self) -> Loaded {
        let path = self.journal_path();
        let mut bytes = Vec::new();
        let read = std::fs::File::open(&path).and_then(|mut f| f.read_to_end(&mut bytes));
        match read {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::Absent,
            // A folder where the file belongs is a shape failure, not a transient one.
            // Windows refuses to open a folder as access denied, so the shape is asked of the path.
            Err(e)
                if e.kind() == std::io::ErrorKind::IsADirectory
                    || std::fs::metadata(&path).is_ok_and(|m| m.is_dir()) =>
            {
                return Loaded::Torn(Torn(e.to_string()), self.stamp());
            }
            Err(e) => return Loaded::Unreadable(e.to_string()),
            Ok(_) => {}
        }
        let stamp = self.stamp();
        // The shim's reading is lossy, so is this one (1796).
        let (text, lossy) = read_lossy(&bytes);
        match (parse(&text), stamp) {
            (Ok(j), Some(s)) => Loaded::Journal(Box::new(j), s, lossy),
            (Ok(j), None) => Loaded::Journal(
                Box::new(j),
                Stamp {
                    len: bytes.len() as u64,
                    mtime: None,
                },
                lossy,
            ),
            (Err(t), s) => Loaded::Torn(t, s),
        }
    }

    /// The torn file renamed `journal.torn-<ms>.json`; its new name, `None` when it could not be moved (1754).
    pub fn set_aside(&self, now_ms: u64) -> Option<String> {
        let to = join(&[&self.dir, &format!("journal.torn-{now_ms}.json")]);
        std::fs::rename(self.journal_path(), &to).ok().map(|()| to)
    }

    /// `journal.json` written atomically; the new stamp (1750).
    pub fn persist(&self, j: &Journal) -> std::io::Result<Stamp> {
        std::fs::create_dir_all(&self.dir)?;
        let text = print(j);
        write_atomic(&self.journal_path(), text.as_bytes())?;
        Ok(self.stamp().unwrap_or(Stamp {
            len: text.len() as u64,
            mtime: None,
        }))
    }

    /// What a crash left: staged pre-images older than `staged_age` and temps of the journal older than an hour (1726).
    pub fn sweep(&self, staged_age: Duration) -> usize {
        self.sweep_except(staged_age, &|_| false)
    }

    /// `sweep`, leaving the files `live` names: a pre-image of a record still running is no crash's.
    pub fn sweep_except(&self, staged_age: Duration, live: &dyn Fn(&str) -> bool) -> usize {
        let old = |meta: &std::fs::Metadata, age: Duration| {
            meta.modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|e| e > age)
        };
        let mut removed = 0;
        for (dir, age, temps_only) in [
            (self.staged_dir(), staged_age, false),
            (self.dir.clone(), TEMP_AGE, true),
        ] {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in read.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if temps_only && !(is_temp(&name) && name.contains("journal.json")) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                // Spelled as the records spell it: a PathBuf's join would use the host's separator.
                let path = join(&[&dir, &name]);
                if meta.is_file()
                    && old(&meta, age)
                    && !live(&path)
                    && std::fs::remove_file(e.path()).is_ok()
                {
                    removed += 1;
                }
            }
        }
        removed
    }
}

/// A store's journal held in memory with the stamp it was read at; every use refreshes it first (1722).
#[derive(Debug)]
pub struct Held {
    pub journal: Journal,
    pub stamp: Option<Stamp>,
    /// Why the file did not read, until a save sets it aside.
    pub torn: Option<Torn>,
    /// Why the file could not be read at all; nothing is saved over it, and every use reads again.
    pub unread: Option<String>,
    /// What reading repaired, counted once per read (1796).
    pub lossy: Lossy,
    /// Whether `take_lossy` has handed `lossy` out since the file was read.
    pub reported: bool,
}

impl Held {
    pub fn read(store: &Store) -> Held {
        match store.load() {
            Loaded::Absent => Held {
                journal: Journal::empty(),
                stamp: None,
                torn: None,
                unread: None,
                lossy: Lossy::default(),
                reported: false,
            },
            Loaded::Journal(j, s, lossy) => Held {
                journal: *j,
                stamp: Some(s),
                torn: None,
                unread: None,
                lossy,
                reported: false,
            },
            Loaded::Torn(t, s) => Held {
                journal: Journal::empty(),
                stamp: s,
                torn: Some(t),
                unread: None,
                lossy: Lossy::default(),
                reported: false,
            },
            Loaded::Unreadable(why) => Held {
                journal: Journal::empty(),
                stamp: None,
                torn: None,
                unread: Some(why),
                lossy: Lossy::default(),
                reported: false,
            },
        }
    }

    /// What reading repaired, once per read of the file (1796).
    pub fn take_lossy(&mut self) -> Lossy {
        if std::mem::replace(&mut self.reported, true) {
            Lossy::default()
        } else {
            self.lossy
        }
    }

    /// Read again when the file moved since; whether it did.
    pub fn refresh(&mut self, store: &Store) -> bool {
        if self.unread.is_none() && store.stamp() == self.stamp {
            return false;
        }
        *self = Held::read(store);
        true
    }

    /// Persist; a torn file is set aside first and its new name answered (1754).
    pub fn save(&mut self, store: &Store, now_ms: u64) -> std::io::Result<Option<String>> {
        if let Some(why) = &self.unread {
            return Err(std::io::Error::other(format!(
                "journal did not read: {why}"
            )));
        }
        let aside = if self.torn.take().is_some() {
            store.set_aside(now_ms)
        } else {
            None
        };
        self.stamp = Some(store.persist(&self.journal)?);
        Ok(aside)
    }
}
