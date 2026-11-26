//! k7-git — a workspace as a git repository over libgit2 (docs/git.md): status, commit, log, identity, and the
//! transports that fetch, push and clone. What a reader gets wrong: libgit2 never filters here (no autocrlf, no
//! LFS) and has no https or ssh of its own; k7 writes blobs and the index itself and brings its own transports.

pub mod auth;
pub mod autosave;
mod changes;
mod commit;
mod error;
mod identity;
pub mod ignore;
pub mod lfs;
mod log;
mod merge;
pub mod paths;
mod remote;
mod repo;
mod transport;

pub use changes::{Change, Entry};
pub use commit::{Cleaned, Cleaner, MAX_FILE, set_default_max_file};
pub use error::Error;
pub use identity::{Identity, IdentityStore};
pub use lfs::{Direct, Landing, Pointer};
pub use log::LogEntry;
pub use merge::{Begin, Choice, Conflict, Kind, Pending, Summary};
pub use repo::{Present, Repo};
pub use transport::{Net, Origin, init, remote_host};

pub type Result<T> = std::result::Result<T, Error>;
