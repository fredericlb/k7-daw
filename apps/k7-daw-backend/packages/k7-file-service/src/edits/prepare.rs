//! prepare.rs — `land_clip_copy` (3j O11): the `optimized` or `proxy` field of a clip, set once its copy exists; the caller holds the gate and the lock.
//! What a reader gets wrong: no route and no copy here: the job that makes the copy is 3f's, the one write it ends with is this.

use k7x::edit::{PathSeg, Value};
use song_edit::clip_edit::clip_file_path;
use song_edit::paths::join;
use song_edit::workspace_doc::ws_data;
use song_edit::yaml_edit::set_file_value;

use super::StagedFs;
use crate::atomic::Real;
use crate::changes::commit::{Landed, commit};

/// Which copy of the clip was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copy {
    Optimized,
    Proxy,
}

impl Copy {
    fn field(self) -> &'static str {
        match self {
            Copy::Optimized => "optimized",
            Copy::Proxy => "proxy",
        }
    }
}

/// Sets `clips/<clip>.k7x`'s field for `kind` to `rel`, the copy's path in the project; whole or not at all.
///
/// # Errors
/// A clip name that is not one plain segment, a clip file that will not read, or the landing's.
/// The caller owes the exclusive journal gate, the write lock, the revision bump and the engine told, as `file_take`'s does.
pub fn land_clip_copy(project: &str, clip: &str, kind: Copy, rel: &str) -> Result<Landed, String> {
    if clip.is_empty() || clip == ".." || clip.contains(['/', '\\']) {
        return Err(format!("not a clip name: {clip}"));
    }
    let mut fs = StagedFs::new(&[project]).map_err(|e| e.to_string())?;
    let file = join(&[project, "clips", &format!("{clip}.k7x")]);
    let path = [PathSeg::Key(kind.field().to_owned())];
    set_file_value(&mut fs, &file, &path, &Value::Str(rel.to_owned()))
        .map_err(|e| e.to_string())?;
    commit(&Real, &fs.planned()?, &mut |_| {}).map_err(|e| e.what)
}

/// The path the clip's file already holds for `kind`, or `None` when it holds none or the file will not read.
#[must_use]
pub fn clip_copy(project: &str, clip: &str, kind: Copy) -> Option<String> {
    if clip.is_empty() || clip == ".." || clip.contains(['/', '\\']) {
        return None;
    }
    let fs = StagedFs::new(&[project]).ok()?;
    let file = clip_file_path(&fs, project, clip).ok()?;
    match ws_data(&fs, &file).ok()?.get(kind.field())? {
        k7x::edit::Value::Str(s) => Some(s.clone()),
        _ => None,
    }
}
