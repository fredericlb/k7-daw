//! k7-roam — the list of workspaces a person added, kept in a per-key store the host provides (iCloud's key-value store
//! on Apple devices) (`docs/roam.md`).
//! What a reader gets wrong: nothing here moves a project's files between devices, git does; the list holds names and remotes.

pub mod entry;
pub mod ffi;
pub mod list;
pub mod mine;
pub mod store;

pub use entry::{Entry, Item};
pub use store::{Fake, ListStore, NoStore};
