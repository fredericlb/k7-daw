//! base_path.rs — `baseSnapshotPathOf`: where a named base goes, judged and not made (2343).
//! What a reader gets wrong: nothing is created here, the folder is the front's to make under the write lock.

use crate::confine::Probe;
use crate::entries::EntryError;
use crate::entries::guards::assert_library_writable;
use crate::entries::plan::{clean_name, is_plain};
use crate::paths::{dirname, join};
use crate::rel::to_rel;
use crate::space::Space;

/// The base's folder and file, absolute, and the workspace path the route answers.
#[derive(Clone, Debug, PartialEq)]
pub struct BasePath {
    pub dir: String,
    pub abs: String,
    pub path: String,
}

/// `base/<name>.vstpreset` beside the module file at `rel`; `warm` and `warm.vstpreset` are one base.
pub fn base_path(
    space: &Space<'_>,
    rel: &str,
    name: &str,
    probe: &dyn Probe,
) -> Result<BasePath, EntryError> {
    let module_abs = space.safe_path(rel, probe)?;
    let clean = clean_name(name);
    if !is_plain(&clean) {
        return Err(EntryError::Refused(format!("invalid name: {name}")));
    }
    let leaf = if clean.ends_with(".vstpreset") {
        clean
    } else {
        format!("{clean}.vstpreset")
    };
    let dir = space.safe_path(
        &to_rel(space, &join(&[&dirname(&module_abs), "base"])),
        probe,
    )?;
    let abs = space.safe_path(&to_rel(space, &join(&[&dir, &leaf])), probe)?;
    assert_library_writable(space, &abs, "snapshot a base into")?;
    Ok(BasePath {
        path: to_rel(space, &abs),
        dir,
        abs,
    })
}
