// yaml_edit.rs — set or clear one field of a workspace file, in place: the FILE primitive, which knows nothing of modules or chains.
// A reader would get wrong the name: the file is k7x, `yaml/edit.ts` kept its old name.

use crate::error::Res;
use crate::fs::Fs;
use crate::workspace_doc::{EditOpts, ws_edit};
use k7x::edit::{PathSeg, Value, WriteOptions};

/// Set `path` in `file`, creating the file and any intermediate maps; a short form is kept while it still holds.
///
/// # Errors
/// What `ws_edit` fails with.
pub fn set_file_value(fs: &mut dyn Fs, file: &str, path: &[PathSeg], value: &Value) -> Res<()> {
    ws_edit(
        fs,
        file,
        EditOpts {
            create: true,
            raw: false,
        },
        |doc| Ok(doc.set_in(path, value, WriteOptions::default())?),
    )?;
    Ok(())
}

/// Remove `path` from `file`; a no-op when it is not there.
///
/// # Errors
/// What `ws_edit` fails with.
pub fn delete_file_value(fs: &mut dyn Fs, file: &str, path: &[PathSeg]) -> Res<()> {
    ws_edit(
        fs,
        file,
        EditOpts {
            create: true,
            raw: false,
        },
        |doc| {
            doc.delete_in(path, false)?;
            Ok(())
        },
    )?;
    Ok(())
}
