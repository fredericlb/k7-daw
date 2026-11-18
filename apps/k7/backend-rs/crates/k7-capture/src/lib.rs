//! k7-capture — the capture of a device's workspaces: what the file service keeps under `<home>/.sync/`, the ledger of a
//! workspace's files, the blob cache and its history. No workspace joins a relay room.
//! What a reader gets wrong: only `NotFound` is absent; any other read error is "not now" and never a default (2102).

pub mod account;
pub mod cache;
pub mod capture;
pub mod confirmed;
pub mod disk;
pub mod failure;
pub mod feed;
pub mod history;
pub mod index;
pub mod landing;
pub mod ledger;
pub mod num;
pub mod registry;
pub mod source;
pub mod worker;

pub use registry::{Captures, native};
