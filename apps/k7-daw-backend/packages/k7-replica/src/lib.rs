//! k7-replica — what the capture keeps of the text a workspace last confirmed: the ops, the blob refs and the `Confirmed`
//! store (`docs/files/replica.md`), with the k7x half behind `K7x`.
//! What a reader gets wrong: no replica lives here any more; the capture's confirmed folder is the one `Confirmed`.

use std::io;

pub mod blob;
pub mod ops;

/// The text this device last confirmed for each file; kept on disk, it is what a restart diffs against.
pub trait Confirmed {
    fn get(&self, file: &str) -> io::Result<Option<String>>;
    fn set(&mut self, file: &str, text: Option<&str>) -> io::Result<()>;
    /// Every file held, in the order they were first set.
    fn files(&self) -> io::Result<Vec<String>>;
}
