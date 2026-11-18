//! container.rs — moves the workspaces left in the iCloud container into the account's own workspaces folder, once.
//! What a reader gets wrong: nothing is overwritten and nothing is deleted before its copy is verified; a workspace
//! that cannot be moved whole (same name here, a placeholder, a failed check) stays where it is and is named in the report.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The file operations the move uses: the seam a test makes fail.
pub trait Ops {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<u64>;
    fn remove_tree(&self, at: &Path) -> io::Result<()>;
}

pub struct Real;

impl Ops for Real {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<u64> {
        std::fs::copy(from, to)
    }
    fn remove_tree(&self, at: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(at)
    }
}

/// Why a workspace was left in the container.
#[derive(Debug, PartialEq, Eq)]
pub enum Why {
    /// A workspace of that name is already here; neither side is touched.
    Exists,
    /// A file iCloud has not downloaded (`.<name>.icloud`, or a dataless file): the folder is not whole.
    NotDownloaded(String),
    /// The copy or its check failed; the container copy is intact.
    Failed(String),
}

#[derive(Debug, Default)]
pub struct Report {
    pub moved: Vec<String>,
    /// Moved, and the container copy could not be fully removed.
    pub leftover: Vec<String>,
    pub kept: Vec<(String, Why)>,
}

impl Report {
    /// One line per workspace, for the log.
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .moved
            .iter()
            .map(|n| format!("{n}: moved out of the iCloud container"))
            .collect();
        out.extend(
            self.leftover
                .iter()
                .map(|n| format!("{n}: moved, the container copy is not fully removed")),
        );
        out.extend(self.kept.iter().map(|(n, w)| match w {
            Why::Exists => {
                format!("{n}: left in the container, a workspace of that name is already here")
            }
            Why::NotDownloaded(f) => format!("{n}: left in the container, {f} is not downloaded"),
            Why::Failed(e) => format!("{n}: left in the container, {e}"),
        }));
        out
    }
}

static SOURCE: OnceLock<PathBuf> = OnceLock::new();

/// The host's container folder, set once at start from `K7_ICLOUD_LEGACY`; a blank value is none.
pub fn set_source(dir: &str) {
    let dir = dir.trim();
    if !dir.is_empty() {
        let _ = SOURCE.set(PathBuf::from(dir));
    }
}

/// `migrate` over the host's container, a log line per workspace; nothing when the host named none.
pub fn migrate_host(target: &Path, say: &dyn Fn(&str)) {
    let Some(source) = SOURCE.get() else { return };
    for line in migrate(source, target).lines() {
        say(&line);
    }
}

/// The marker folder of the capture, one small file per workspace.
const MARKERS: &str = ".active-projects";
const SCRATCH: &str = "migrating";

/// Moves every workspace folder of `source` into `target`.
pub fn migrate(source: &Path, target: &Path) -> Report {
    migrate_with(&Real, source, target)
}

pub fn migrate_with(ops: &dyn Ops, source: &Path, target: &Path) -> Report {
    let mut report = Report::default();
    let Ok(read) = std::fs::read_dir(source) else {
        return report;
    };
    let mut names: Vec<String> = read
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        let at = source.join(&name);
        if name == MARKERS {
            markers(ops, &at, &target.join(MARKERS));
            continue;
        }
        if let Some(hidden) = name
            .strip_prefix('.')
            .and_then(|n| n.strip_suffix(".icloud"))
        {
            report
                .kept
                .push((hidden.to_string(), Why::NotDownloaded(name.clone())));
            continue;
        }
        if name.starts_with('.') || !std::fs::symlink_metadata(&at).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        match one(ops, &at, target, &name) {
            Ok(true) => report.moved.push(name),
            Ok(false) => report.leftover.push(name),
            Err(why) => report.kept.push((name, why)),
        }
    }
    report
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Dir,
    File(u64),
    Link(PathBuf),
}

type Manifest = BTreeMap<PathBuf, Kind>;

/// Whether the flags of a file say iCloud holds its bytes and this device does not (`SF_DATALESS`).
pub fn is_dataless(flags: u32) -> bool {
    flags & 0x4000_0000 != 0
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn dataless(m: &std::fs::Metadata) -> bool {
    #[cfg(target_os = "ios")]
    use std::os::ios::fs::MetadataExt;
    #[cfg(target_os = "macos")]
    use std::os::macos::fs::MetadataExt;
    is_dataless(m.st_flags())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn dataless(_: &std::fs::Metadata) -> bool {
    false
}

/// Every entry below `root` with its kind and size, links not followed; a file not downloaded is an error naming it.
fn manifest(root: &Path) -> Result<Manifest, Why> {
    fn walk(root: &Path, dir: &Path, out: &mut Manifest) -> Result<(), Why> {
        let fail = |e: io::Error| Why::Failed(format!("{}: {e}", dir.display()));
        for e in std::fs::read_dir(dir).map_err(fail)? {
            let e = e.map_err(fail)?;
            let (path, name) = (e.path(), e.file_name().to_string_lossy().into_owned());
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if name.starts_with('.') && name.ends_with(".icloud") {
                return Err(Why::NotDownloaded(rel.display().to_string()));
            }
            let m = std::fs::symlink_metadata(&path).map_err(fail)?;
            if m.file_type().is_symlink() {
                let to = std::fs::read_link(&path).map_err(fail)?;
                out.insert(rel, Kind::Link(to));
            } else if m.is_dir() {
                out.insert(rel, Kind::Dir);
                walk(root, &path, out)?;
            } else if dataless(&m) {
                return Err(Why::NotDownloaded(rel.display().to_string()));
            } else {
                out.insert(rel, Kind::File(m.len()));
            }
        }
        Ok(())
    }
    let mut out = Manifest::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

#[cfg(unix)]
fn link(to: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(to, at)
}

#[cfg(not(unix))]
fn link(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::other("no link here"))
}

fn copy_tree(ops: &dyn Ops, from: &Path, to: &Path, man: &Manifest) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for (rel, kind) in man {
        let (a, b) = (from.join(rel), to.join(rel));
        match kind {
            Kind::Dir => std::fs::create_dir_all(&b)?,
            Kind::File(_) => {
                ops.copy_file(&a, &b)?;
            }
            Kind::Link(target) => link(target, &b)?,
        }
    }
    Ok(())
}

/// Reads every object of the repository whose git folder is `git`: libgit2 checks each one's hash as it reads.
fn fsck(git: &Path) -> Result<(), String> {
    let repo =
        git2::Repository::open(git).map_err(|e| format!("the repository does not open: {e}"))?;
    let odb = repo.odb().map_err(|e| e.to_string())?;
    let mut bad = None;
    odb.foreach(|oid| {
        if let Err(e) = odb.read(*oid) {
            bad = Some(format!("object {oid}: {e}"));
            return false;
        }
        true
    })
    .map_err(|e| e.to_string())?;
    bad.map_or(Ok(()), |b| Err(format!("the repository check failed, {b}")))
}

/// The git folder of a copied workspace, whether `.git` is the folder or the old link to `.git.nosync`.
fn git_folder(root: &Path) -> Option<PathBuf> {
    let dot = root.join(".git");
    match std::fs::symlink_metadata(&dot) {
        Ok(m) if m.is_dir() => Some(dot),
        Ok(_) => Some(root.join(".git.nosync")).filter(|p| p.is_dir()),
        Err(_) => Some(root.join(".git.nosync")).filter(|p| p.is_dir()),
    }
}

/// `Ok(true)` moved whole, `Ok(false)` moved with the container copy not fully removed.
fn one(ops: &dyn Ops, src: &Path, target: &Path, name: &str) -> Result<bool, Why> {
    let dest = target.join(name);
    if std::fs::symlink_metadata(&dest).is_ok() {
        return Err(Why::Exists);
    }
    let man = manifest(src)?;
    std::fs::create_dir_all(target)
        .map_err(|e| Why::Failed(format!("{}: {e}", target.display())))?;
    if ops.rename(src, &dest).is_ok() {
        let _ = k7_git::restore_gitdir(&dest);
        return Ok(true);
    }
    let scratch = target.join(format!(".{name}.{SCRATCH}"));
    let _ = ops.remove_tree(&scratch);
    let undo = |why: Why| {
        let _ = ops.remove_tree(&scratch);
        why
    };
    copy_tree(ops, src, &scratch, &man)
        .map_err(|e| undo(Why::Failed(format!("the copy failed: {e}"))))?;
    match manifest(&scratch) {
        Ok(got) if got == man => {}
        Ok(_) => {
            return Err(undo(Why::Failed(
                "the copy differs from the original in its files or sizes".into(),
            )));
        }
        Err(w) => return Err(undo(w)),
    }
    if let Some(git) = git_folder(&scratch) {
        fsck(&git).map_err(|e| undo(Why::Failed(e)))?;
    }
    if std::fs::symlink_metadata(&dest).is_ok() {
        return Err(undo(Why::Exists));
    }
    ops.rename(&scratch, &dest).map_err(|e| {
        undo(Why::Failed(format!(
            "the copy could not be put in place: {e}"
        )))
    })?;
    let _ = k7_git::restore_gitdir(&dest);
    Ok(ops.remove_tree(src).is_ok())
}

/// The capture's markers: each file moves only when this device has none of that name.
fn markers(ops: &dyn Ops, from: &Path, to: &Path) {
    let Ok(read) = std::fs::read_dir(from) else {
        return;
    };
    for e in read.flatten() {
        let (a, b) = (e.path(), to.join(e.file_name()));
        let file = std::fs::symlink_metadata(&a).is_ok_and(|m| m.is_file() && !dataless(&m));
        if !file || std::fs::symlink_metadata(&b).is_ok() || std::fs::create_dir_all(to).is_err() {
            continue;
        }
        let size = std::fs::metadata(&a).map(|m| m.len()).ok();
        if ops.copy_file(&a, &b).is_ok() && std::fs::metadata(&b).map(|m| m.len()).ok() == size {
            let _ = std::fs::remove_file(&a);
        } else {
            let _ = std::fs::remove_file(&b);
        }
    }
}
