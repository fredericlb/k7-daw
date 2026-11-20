//! k7-libraries — a machine's libraries in Rust: the store of folders, versions, seeds and manifests, and the routes
//! behind the hub's secret (`docs/libraries.md`).
//! What a reader gets wrong: the folders' layout is file-hub's byte for byte — a library rewritten on update would
//! re-sync every device — and a name or a version from a request never becomes a path before it is checked.

mod copy;
mod dirs;
mod list;
mod manifest;
mod reply;
mod routes;
mod store;

pub use copy::copy_tree;
pub use dirs::{MANIFEST, is_husk, publishable, version_numbers};
pub use list::Note;
pub use manifest::{Manifest, parse_manifest, stringify_manifest};
pub use reply::{Body, Reply};
pub use routes::{JSON_BODY_MAX, Libraries, Request};
pub use store::{DEFAULT_LIBRARY, Store, parse_seed, settings_problem};
