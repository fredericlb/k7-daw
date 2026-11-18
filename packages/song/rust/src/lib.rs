// lib.rs — the song compiler: a Source to read through, a cache of decoded files, the file readers, resolution, routing, the ProjectDoc.
// A reader would get wrong that this crate reaches a route: only k7-file-service's song sessions call `session`.
// No FFI and no `std::fs` outside source/disk.rs; the `disk` feature is the only file system code (docs/rustCompiler.md).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

/// The sub-step this crate has reached: a stage-gate entry may only wait for a later one.
pub const STAGE: &str = "2l";

pub mod assemblies;
pub mod cache;
pub mod clips;
pub mod compare;
pub mod console;
pub mod curves;
pub mod doc;
pub mod dsp_literal;
pub mod files;
pub mod freeze;
pub mod graph;
pub mod jsdsp;
pub mod jsorder;
pub mod lanes;
pub mod modules;
pub mod pictures;
pub mod read;
pub mod readset;
pub mod record;
pub mod resolve;
pub mod roots;
pub mod routing;
pub mod send_refs;
pub mod session;
pub mod source;
pub mod templates;
pub mod views;

pub use cache::{Cache, FileValue, Read};
pub use files::{
    AudioClip, CaptionClip, Clip, Finding, Marker, Note, Placement, ProjectFile, ProjectSettings, RootFile, Sequence,
    Settings, SettingsDoc, Severity, Timeline, VideoClip,
};
pub use modules::{LoadedModule, Module, Vocabulary};
pub use read::{FamilyRead, Head, read_family, read_head};
pub use readset::{ReadSet, Watch};
pub use resolve::{Resolution, Resolved, resolve_map, resolve_ref};
pub use roots::{Library, Roots};
pub use routing::{Routing, load_routing};
#[cfg(feature = "disk")]
pub use source::DiskSource;
pub use source::path::PathRule;
pub use source::{
    BundleFile, BundleRequest, Entry, Kind, MAX_BUNDLE, MAX_ENTRIES, MAX_MOUNTS, MAX_PATH_DEPTH, MAX_PATH_LEN,
    MemSource, Source, SourceBundle, SourceError, Span, Stamp, Stat,
};
