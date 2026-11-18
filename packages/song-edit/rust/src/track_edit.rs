// track_edit.rs — edits one lane's file in place through a `Scope` on its root, whichever folder the lane lives in.
// A reader would get wrong where a lane is: `nodes/` or a chain project's `tracks/`, or a timeline's lane inside the timeline file.

use crate::chain_item_edit::{lane_events_at, lane_folder};
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::workspace_doc::{EditOpts, ws_edit};
use k7x::edit::Scope;

/// Edits lane `id`'s file in place through `edit` on its root; `what` names the edit in the refusal.
///
/// # Errors
/// A lane with no file, what `edit` fails with, or the write's.
pub fn edit_track_struct(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    what: &str,
    edit: impl FnOnce(&mut Scope) -> Res<()>,
) -> Res<String> {
    let Some(found) = lane_events_at(fs, dir, id)? else {
        return fail(format!(
            "track \"{id}\" has no file in {}/ — cannot {what}",
            lane_folder(fs, dir)
        ));
    };
    ws_edit(fs, &found.path, EditOpts::default(), |doc| {
        edit(&mut doc.at(&found.base))
    })?;
    Ok(found.path)
}
