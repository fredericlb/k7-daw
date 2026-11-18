//! error.rs — what the repository refuses with: a code for the screen and a sentence, never a secret.
//! What a reader gets wrong: libgit2's own message is carried as text; no variant holds a credential, and a
//! typed failure of a transport (an unknown host key, a refused token) is kept apart from libgit2's string.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// libgit2 refused; its sentence.
    Git(String),
    /// The workspace's files.
    Io(String),
    NoIdentity,
    NothingToCommit,
    /// A file over `MAX_FILE`, refused at commit.
    TooBig {
        path: String,
        size: u64,
    },
    BadPath(String),
    /// A path that differs from a tracked one by case alone, on a volume that reads them as one file.
    CaseClash {
        theirs: String,
        mine: String,
    },
    /// No credential, or the server refused it; the sentence says which.
    Auth(String),
    /// The remote has commits this branch lacks: pull first.
    Behind,
    /// The server's key is not on the trusted list; the person decides.
    UnknownHost {
        fingerprint: String,
    },
    /// The server's key differs from the trusted one; nothing was sent.
    HostKeyChanged {
        expected: String,
        got: String,
    },
    BadUrl(String),
    /// An LFS transfer over `http://` away from loopback.
    InsecureUrl,
    /// Any remote that is not an ssh address (`https://`, `http://`, `git://`, `file://`, a local path).
    SshOnly,
    /// The server could not be reached, or answered what no git server does.
    Network(String),
    BadKey(String),
    /// A clone's target is taken.
    Exists(String),
    /// The host has no LFS: nothing was sent.
    NoLfs {
        host: String,
    },
    /// The host answered the large-file request with a refusal, in its own words.
    LfsRefused(String),
    /// The LFS server answered a real batch with a status that is no grant and no refusal of the token.
    LfsStatus {
        status: u16,
        url: String,
    },
    /// Changes not committed: a pull commits them first.
    Uncommitted,
    /// A merge is already waiting for its choices.
    MergeOpen,
    /// There is no merge waiting.
    NoMerge,
    /// The branch moved while the merge waited.
    MergeMoved,
    /// Conflicts still have no choice.
    Unresolved {
        n: usize,
    },
    /// A `.k7x` the merge would write does not read; the parser's sentence names the line.
    Unparsable {
        path: String,
        message: String,
    },
    /// Another git operation holds the workspace.
    Busy,
    /// A large file's bytes are not here and the host cannot be reached.
    Offline {
        path: String,
    },
}

impl Error {
    /// The code the routes answer with (docs/git.md).
    pub fn code(&self) -> &'static str {
        match self {
            Error::Git(_) => "git",
            Error::Io(_) => "internal",
            Error::NoIdentity => "no-identity",
            Error::NothingToCommit => "nothing-to-commit",
            Error::TooBig { .. } => "too-big",
            Error::BadPath(_) => "bad-path",
            Error::CaseClash { .. } => "case-clash",
            Error::Auth(_) => "unauthorized",
            Error::Behind => "behind",
            Error::UnknownHost { .. } => "unknown-host",
            Error::HostKeyChanged { .. } => "host-key-changed",
            Error::BadUrl(_) => "bad-request",
            Error::SshOnly => "ssh-only",
            Error::InsecureUrl => "insecure",
            Error::Network(_) => "unreachable",
            Error::BadKey(_) => "bad-key",
            Error::Exists(_) => "exists",
            Error::NoLfs { .. } => "no-lfs",
            Error::LfsRefused(_) => "lfs-refused",
            Error::LfsStatus { .. } => "lfs-status",
            Error::Offline { .. } => "offline",
            Error::Busy => "busy",
            Error::Uncommitted => "uncommitted",
            Error::MergeOpen => "merge-open",
            Error::NoMerge => "no-merge",
            Error::MergeMoved => "merge-moved",
            Error::Unresolved { .. } => "unresolved",
            Error::Unparsable { .. } => "unparsable",
        }
    }

    /// The HTTP status a route gives it.
    pub fn status(&self) -> u16 {
        match self {
            Error::Io(_) => 500,
            Error::Auth(_) => 401,
            Error::NothingToCommit | Error::Behind | Error::Exists(_) => 409,
            Error::Uncommitted
            | Error::MergeOpen
            | Error::MergeMoved
            | Error::Unresolved { .. }
            | Error::Busy => 409,
            Error::CaseClash { .. } => 409,
            Error::NoMerge => 404,
            Error::UnknownHost { .. } | Error::HostKeyChanged { .. } => 409,
            Error::Network(_) | Error::Git(_) | Error::NoLfs { .. } | Error::LfsStatus { .. } => {
                502
            }
            Error::LfsRefused(_) => 403,
            Error::Offline { .. } => 503,
            _ => 400,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Git(m) => write!(f, "git: {m}"),
            Error::Io(m) => write!(f, "the workspace's files: {m}"),
            Error::NoIdentity => f.write_str("set a name and an email before committing"),
            Error::NothingToCommit => f.write_str("nothing changed since the last commit"),
            Error::TooBig { path, size } => write!(
                f,
                "{path} is {} MB, over the 2 GB a git host takes; keep it out of the workspace",
                size / 1_000_000
            ),
            Error::BadPath(p) => write!(f, "{p} is not a path k7 commits"),
            Error::CaseClash { theirs, mine } => write!(
                f,
                "{theirs} would replace {mine} on this disk, which reads names without case: rename one of them, then pull again"
            ),
            Error::Auth(m) => f.write_str(m),
            Error::Behind => f.write_str("the remote has changes this workspace lacks: pull first"),
            Error::UnknownHost { fingerprint } => {
                write!(
                    f,
                    "this server is not trusted yet; its key is {fingerprint}"
                )
            }
            Error::HostKeyChanged { expected, got } => write!(
                f,
                "the server's key changed (trusted {expected}, shown {got}); nothing was sent"
            ),
            Error::BadUrl(m) => write!(f, "this address cannot be used: {m}"),
            Error::InsecureUrl => {
                f.write_str("a large-file transfer over http:// is only accepted for this machine")
            }
            Error::SshOnly => f.write_str(
                "k7 syncs over SSH only: use an ssh address such as git@host:owner/repo.git or ssh://git@host/owner/repo.git",
            ),
            Error::Network(m) => f.write_str(m),
            Error::BadKey(m) => f.write_str(m),
            Error::Exists(p) => write!(f, "{p} already exists"),
            Error::NoLfs { host } => write!(
                f,
                "{host} does not store large files (git LFS); nothing was sent. Use a host with LFS, or keep the audio out of this workspace"
            ),
            Error::LfsRefused(m) => write!(f, "the host refused large-file access: {m}"),
            Error::LfsStatus { status, url } => {
                write!(f, "the LFS server at {url} answered {status}; nothing was sent")
            }
            Error::Uncommitted => {
                f.write_str("commit your changes first: a pull needs a committed workspace")
            }
            Error::MergeOpen => {
                f.write_str("a merge is waiting for your choices: finish or abort it first")
            }
            Error::NoMerge => f.write_str("there is no merge waiting"),
            Error::MergeMoved => {
                f.write_str("the branch moved while the merge waited: abort it and pull again")
            }
            Error::Unresolved { n } => write!(f, "{n} conflict(s) still need a choice"),
            Error::Unparsable { path, message } => {
                write!(f, "{path} would not read after this choice: {message}")
            }
            Error::Busy => f.write_str(
                "another git operation is running on this workspace: try again when it is done",
            ),
            Error::Offline { path } => write!(
                f,
                "{path} is not on this device and the server cannot be reached"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Error {
        Error::Git(e.message().to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e.to_string())
    }
}
