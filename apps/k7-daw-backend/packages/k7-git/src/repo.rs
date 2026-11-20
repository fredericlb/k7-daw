//! repo.rs — a workspace folder and its git folder, opened together: the git folder is `<workdir>/.git` or a
//! folder elsewhere (a device's own). What a reader gets wrong: opening a git folder
//! elsewhere writes no `.git` file into the workspace, and libgit2 is a blocking library: call from a blocking thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::{Repository, RepositoryInitOptions};

use crate::{Result, ignore};

/// Whether a file is on this device: a placeholder or an evicted file is present-but-not-here, never a removal.
pub trait Present: Send + Sync {
    fn present(&self, rel: &str) -> bool;
}

struct Always;

impl Present for Always {
    fn present(&self, _: &str) -> bool {
        true
    }
}

pub struct Repo {
    pub(crate) git: Repository,
    pub(crate) workdir: PathBuf,
    pub(crate) gitdir: PathBuf,
    pub(crate) present: Arc<dyn Present>,
    pub(crate) max_file: u64,
}

impl Repo {
    /// The repository whose files are `workdir` and whose git folder is `gitdir`.
    pub fn open(workdir: &Path, gitdir: &Path) -> Result<Repo> {
        let git = Repository::open(gitdir)?;
        git.set_workdir(workdir, false)?;
        Repo::wrap(git, workdir, gitdir)
    }

    /// A new repository on the branch `main`, k7's ignore list written, nothing committed.
    pub fn init(workdir: &Path, gitdir: &Path) -> Result<Repo> {
        let repo = Repo::make(workdir, gitdir)?;
        ignore::write_gitignore(workdir)?;
        crate::lfs::attributes::ensure(workdir)?;
        Ok(repo)
    }

    /// A repository about to receive a clone: the ignore list only in `info/exclude`, the remote's own `.gitignore` kept.
    pub(crate) fn init_clone(workdir: &Path) -> Result<Repo> {
        Repo::make(workdir, &workdir.join(".git"))
    }

    fn make(workdir: &Path, gitdir: &Path) -> Result<Repo> {
        std::fs::create_dir_all(workdir)?;
        let mut o = RepositoryInitOptions::new();
        o.initial_head("main").external_template(false).mkpath(true);
        let at = if gitdir == workdir.join(".git") {
            workdir
        } else {
            o.workdir_path(workdir).no_dotgit_dir(true);
            gitdir
        };
        let git = Repository::init_opts(at, &o)?;
        // libgit2 links a foreign git folder with a `.git` file; the workspace never carries one (core.worktree does).
        if at != workdir {
            let link = workdir.join(".git");
            if link.is_file() {
                std::fs::remove_file(link)?;
            }
        }
        Repo::wrap(git, workdir, gitdir)
    }

    fn wrap(git: Repository, workdir: &Path, gitdir: &Path) -> Result<Repo> {
        // A person's global autocrlf must never rewrite an audio file or a `.k7x`.
        let mut cfg = git.config()?;
        cfg.set_bool("core.autocrlf", false)?;
        // A checked-out link would carry a path out of the workspace; it lands as a small file holding its target.
        cfg.set_bool("core.symlinks", false)?;
        ignore::write_exclude(gitdir)?;
        Ok(Repo {
            git,
            workdir: workdir.to_path_buf(),
            gitdir: gitdir.to_path_buf(),
            present: Arc::new(Always),
            max_file: crate::commit::default_max_file(),
        })
    }

    /// The device's answer to "is this file here" (I2); every file is until it is set.
    pub fn set_present(&mut self, present: Arc<dyn Present>) {
        self.present = present;
    }

    /// Test seam: the size over which a file stays out of a commit.
    pub fn set_max_file(&mut self, bytes: u64) {
        self.max_file = bytes;
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn gitdir(&self) -> &Path {
        &self.gitdir
    }

    /// The branch `HEAD` names, even before its first commit.
    pub fn branch(&self) -> Option<String> {
        let head = self.git.find_reference("HEAD").ok()?;
        let target = head.symbolic_target().ok().flatten()?;
        target.strip_prefix("refs/heads/").map(str::to_string)
    }
}
