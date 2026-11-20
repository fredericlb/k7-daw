//! stamp.rs — the format stamp: `format(N)` raised in project.k7x once a lane places an event in seconds (O2), `stampFormatFor`'s port.
//! What a reader gets wrong: best-effort, as the TypeScript is: a file that will not read stamps nothing and is the compile's to report.

use song_edit::format_level::SECONDS_START_LEVEL;
use song_edit::format_stamp::{holds_seconds_starts, stamp_format_for_seconds, stamp_format_level};
use song_edit::workspace_doc::ws_data;

use super::DiskFs;
use crate::filter::project_app;
use crate::paths::{basename, dirname};

/// The project folder holding `abs`, walking up to `root`; None when no folder on the way is a project.
fn project_of(root: &str, abs: &str) -> Option<String> {
    let mut dir = dirname(abs);
    while project_app(&basename(&dir)).is_none() {
        if dir == root || dirname(&dir) == dir {
            return None;
        }
        dir = dirname(&dir);
    }
    Some(dir)
}

/// A written `.k7x` placing an event in seconds raises its project's level; true when it wrote.
#[must_use]
pub fn stamp_format_for(root: &str, abs: &str) -> bool {
    if !abs.ends_with(".k7x") {
        return false;
    }
    let Some(dir) = project_of(root, abs) else {
        return false;
    };
    let Ok(mut fs) = DiskFs::new(&[dir.as_str()]) else {
        return false;
    };
    ws_data(&fs, abs).is_ok_and(|d| holds_seconds_starts(&d))
        && stamp_format_level(&mut fs, &dir, SECONDS_START_LEVEL)
}

/// The open's stamp: the level seconds need when any lane or timeline of `dir` writes one; true when it wrote.
#[must_use]
pub fn stamp_format_at_open(dir: &str) -> bool {
    DiskFs::new(&[dir]).is_ok_and(|mut fs| stamp_format_for_seconds(&mut fs, dir))
}
