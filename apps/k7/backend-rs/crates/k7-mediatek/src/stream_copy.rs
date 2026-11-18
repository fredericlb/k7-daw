//! stream_copy.rs — a FLAC's Opus copy, the WebM beside it:
//! the encode, the part file and the checks (`packages/audio-analysis/docs/transcode.md`), without the queue, the caller's.
//! What a reader gets wrong: a FLAC replaced while it was read leaves no copy, and says so, so the caller asks again.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::ecma::mtime_ms;
use crate::media::derived_of;
use crate::transcode::{Opus, Piece, transcode};
use crate::waveform::Waveform;

/// An encode past this is stuck, not slow: a two-hour FLAC takes well under a minute.
pub const ENCODE_TIME: Duration = Duration::from_secs(10 * 60);

/// Opus at 128 kbit/s: transparent for listening, and a fifth of a FLAC.
const BITRATE: u32 = 128_000;

/// Why no copy was made.
#[derive(Clone, Debug, PartialEq)]
pub enum CopyError {
    /// The FLAC changed or went away while it was read (`SourceChangedError`): nothing is wrong with the file.
    SourceChanged,
    /// Anything else: not a FLAC, unreadable, or the encode failed.
    Failed(String),
}

/// The copy beside `flac`, absolute, or None for a name that is no FLAC.
pub fn copy_path(flac: &Path) -> Option<PathBuf> {
    let name = flac.file_name()?.to_str()?;
    Some(flac.with_file_name(derived_of(name)?))
}

fn mtime_of(p: &Path) -> Option<f64> {
    std::fs::metadata(p).ok().map(|m| mtime_ms(&m))
}

/// `copyIsFresh`: the copy is there and at least as new as its FLAC; a replaced FLAC answers false.
pub fn copy_is_fresh(flac: &Path) -> bool {
    let Some(copy) = copy_path(flac) else {
        return false;
    };
    matches!((mtime_of(&copy), mtime_of(flac)), (Some(c), Some(f)) if c >= f)
}

/// `encode`: the copy written beside the FLAC at `abs` through a part file, then checked against the FLAC's time.
pub fn make_copy(waveform: &Waveform, abs: &Path) -> Result<PathBuf, CopyError> {
    let dest = copy_path(abs)
        .ok_or_else(|| CopyError::Failed(format!("{} is not a FLAC", abs.display())))?;
    let before =
        mtime_of(abs).ok_or_else(|| CopyError::Failed(format!("{}: not found", abs.display())))?;
    let deadline = Instant::now() + ENCODE_TIME;
    let stop = |src: &Path| {
        (Instant::now() > deadline)
            .then(|| format!("{}: the encode ran past its deadline", src.display()))
    };
    let opus = Opus {
        bitrate: BITRATE,
        channels: 0,
    };
    let written = transcode(
        waveform,
        &[Piece::whole(abs.to_path_buf())],
        &dest,
        &opus,
        None,
        &stop,
    );
    written.map_err(CopyError::Failed)?;
    // A FLAC replaced, moved or removed while it was read: the copy is of the old take, or would list as a file of its own.
    if mtime_of(abs) != Some(before) {
        let _ = std::fs::remove_file(&dest);
        return Err(CopyError::SourceChanged);
    }
    Ok(dest)
}
