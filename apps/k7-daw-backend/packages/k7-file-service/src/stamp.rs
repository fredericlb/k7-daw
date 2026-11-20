//! The compiler's version this backend reports, read by `build.rs` from the checked-in module of the browser's crate.
//! What a reader gets wrong: it is not computed here: `SOURCES` is the one value the module, the facade and this backend share.

include!(concat!(env!("OUT_DIR"), "/sources.rs"));
