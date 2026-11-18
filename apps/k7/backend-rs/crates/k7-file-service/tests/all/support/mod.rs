//! Shared by the tests: a scratch folder removed when dropped, a home inside it, a fake disk that counts what it is asked.

#![allow(dead_code)]

pub mod chains;
pub mod changes;
pub mod engine;
pub mod entries;
pub mod jobs;
pub mod journal;
pub mod media;
pub mod projects;
pub mod ws;
pub mod wscopy;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use k7_file_service::confine::Probe;
use k7_file_service::libraries::Home;

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        // The process id keeps two runs apart; the name is what a failure shows.
        let dir = std::env::temp_dir().join(format!("k7-fs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = std::fs::canonicalize(dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        // Windows' canonical spelling `\\?\C:\` is no path of the model's.
        Scratch(PathBuf::from(real.strip_prefix(r"\\?\").unwrap_or(&real)))
    }

    pub fn path(&self, rel: &str) -> String {
        k7_file_service::paths::join(&[&self.0.to_string_lossy().replace('\\', "/"), rel])
    }

    /// `rel` spelt as given, for a name the path model would rewrite (a backslash).
    pub fn raw(&self, rel: &str) -> String {
        format!("{}/{rel}", self.0.to_string_lossy())
    }

    pub fn dir(&self, rel: &str) -> String {
        let p = self.path(rel);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn file(&self, rel: &str, text: &str) -> String {
        let p = self.path(rel);
        if let Some(parent) = Path::new(&p).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, text).unwrap();
        p
    }

    pub fn home(&self, admin: bool) -> Home {
        Home {
            studio: self.path("studio"),
            libraries: self.path("libraries"),
            admin,
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Makes a symbolic link to a folder, where this host lets it (Windows needs a privilege).
pub fn link(target: &str, at: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, at);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, at);
}

/// Makes a symbolic link to a file: Windows types a link, and a folder link to a file opens nothing.
pub fn link_file(target: &str, at: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, at);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(target, at);
}

/// Whether this host can make such a link; a link test returns early where it cannot.
pub fn can_link(s: &Scratch) -> bool {
    s.dir("probe-target");
    link(&s.path("probe-target"), &s.path("probe-link")).is_ok()
}

/// True where the tests run as root, which `chmod 000` does not stop.
#[cfg(unix)]
pub fn is_root() -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| o.stdout == b"0\n")
}

/// A disk that answers from tables and counts what it was asked.
#[derive(Default)]
pub struct Fake {
    pub present: HashSet<String>,
    pub real: HashMap<String, String>,
    pub asked: AtomicUsize,
}

impl Fake {
    pub fn with(present: &[&str], real: &[(&str, &str)]) -> Fake {
        Fake {
            present: present.iter().map(|s| s.to_string()).collect(),
            real: real
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            asked: AtomicUsize::new(0),
        }
    }
}

impl Probe for Fake {
    fn present(&self, p: &str) -> bool {
        self.asked.fetch_add(1, Ordering::Relaxed);
        self.present.contains(p)
    }

    fn real(&self, p: &str) -> Option<String> {
        self.real
            .get(p)
            .cloned()
            .or_else(|| self.present.contains(p).then(|| p.to_string()))
    }
}
