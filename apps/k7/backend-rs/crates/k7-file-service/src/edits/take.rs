//! take.rs — a finished take filed (3j O1): staged through song-edit, landed whole; the caller holds the account's write lock.
//! What a reader gets wrong: the tempo, the metre and the stamp are arguments, so the goldens fix them; no journal step is made here.

use song_edit::record_take::{RecordedTake, TakeTarget, add_take_to_event, add_take_to_track};

use super::StagedFs;
use crate::atomic::Real;
use crate::changes::commit::{Landed, commit};
use crate::engine::docs::TakeArm;
use crate::engine::wire::Take;
use crate::paths::{dirname, join, slashed};
use song_edit::paths::resolve;

pub use song_edit::record_take::Meter;

/// The take renamed, declared and placed as one landing: the clip's id and what landed. The arm sends it into an event when it names its track.
///
/// # Errors
/// What the edit says (a file gone, a lane with no file, no event at the arm), or the landing's; nothing is written then.
pub fn file_take(
    project: &str,
    take: &Take,
    arm: Option<&TakeArm>,
    meter: Meter,
    stamp: &str,
) -> Result<(String, Landed), String> {
    // The engine records into the song's `clips/` and the clip is declared from there: a take anywhere else is refused.
    let file = resolve(&slashed(&take.file));
    if dirname(&file) != join(&[project, "clips"]) {
        return Err(format!(
            "recorded file is not in the project's clips folder: {}",
            take.file
        ));
    }
    let mut fs = StagedFs::new(&[project]).map_err(|e| e.to_string())?;
    let taken = RecordedTake {
        track: take.track.clone(),
        file,
        start: take.start,
        length: take.length,
    };
    let id = match arm.filter(|a| a.track == take.track) {
        Some(a) => {
            let target = TakeTarget {
                event_beats: a.event_beats,
                punch: Some((a.from, a.to)),
            };
            add_take_to_event(&mut fs, project, meter, &taken, &target, stamp)
        }
        None => add_take_to_track(&mut fs, project, meter, &taken, stamp),
    }
    .map_err(|e| e.to_string())?;
    let landed = commit(&Real, &fs.planned()?, &mut |_| {}).map_err(|e| e.what)?;
    Ok((id, landed))
}
