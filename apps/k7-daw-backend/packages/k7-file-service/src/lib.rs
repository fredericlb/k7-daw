//! k7-file-service — the file service's native reads: the path model, the two gates, the libraries' roots, a
//! file's text and the workspace tree. Plain functions over a folder: no HTTP, no runtime of its own.
//! What a reader gets wrong: the lexical model is node's `path`, not `std::path` — docs/file-native.md.

pub mod atomic;
pub mod chains;
pub mod changes;
pub mod confine;
pub mod edits;
pub mod engine;
pub mod entries;
pub mod filter;
pub mod jobs;
pub mod journal;
pub mod libraries;
pub mod listings;
pub mod media;
pub mod module_reads;
pub mod modwatch;
pub mod paths;
pub mod pins;
pub mod preferences;
pub mod project_reads;
pub mod projects;
pub mod read;
pub mod rel;
pub mod revision;
pub mod revisions;
pub mod rig;
pub mod runtime;
pub mod shares;
pub mod song_ask;
pub mod songs;
pub mod space;
pub mod stamp;
pub mod strict;
pub mod text_hash;
pub mod tree;
pub mod verb_host;
pub mod verb_paths;
pub mod watch;
pub mod watcher;
pub mod workspaces;
// 3j M5: the bench and the slot.
pub mod bench;
