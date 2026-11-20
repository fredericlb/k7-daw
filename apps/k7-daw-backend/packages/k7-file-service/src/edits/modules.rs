//! modules.rs — a library module's ref IS its path: the repoint of every ref a rename or move strands, and the move's plan.
//! What a reader gets wrong: the repoint lands whole or not at all, but it runs AFTER the folder moved and never moves it back.

use song_edit::module_move::{Library as MoveLibrary, plan_module_move, repoint_refs};
use song_edit::paths::resolve;

use super::{StagedFs, posix, real};
use crate::atomic::Real;
use crate::changes::commit::commit;
use crate::confine::Disk;
use crate::entries::EntryError;
use crate::entries::guards::{assert_library_writable, guard_mutable};
use crate::libraries::library_of;
use crate::rel::to_rel;
use crate::space::Space;

/// What a library move will do: the folder's new place and the two refs it trades.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryMove {
    pub root: String,
    pub from_abs: String,
    pub dest: String,
    pub from: String,
    pub to: String,
}

/// Every ref `from` rewritten to `to` under `roots` (`under`: and every ref below it), landed as one set; the files changed, absolute.
///
/// # Errors
/// A file that cannot be read, or the landing's failure, with nothing written.
pub fn repoint(roots: &[String], from: &str, to: &str, under: bool) -> Result<Vec<String>, String> {
    let posixed: Vec<String> = roots.iter().map(|r| posix(r)).collect();
    let mut fs = StagedFs::new(&posixed).map_err(|e| e.to_string())?;
    let changed = repoint_refs(&mut fs, &posixed, from, to, under).map_err(|e| e.to_string())?;
    if changed.is_empty() {
        return Ok(Vec::new());
    }
    let set = fs.planned()?;
    commit(&Real, &set, &mut |_| {}).map_err(|e| e.what)?;
    Ok(changed.iter().map(|f| real(f)).collect())
}

fn refused(s: String) -> EntryError {
    EntryError::Refused(s)
}

/// `moveLibraryModuleIn`'s judging, in its order: nothing is written.
///
/// # Errors
/// The first guard or plan failure, as the route words it.
pub fn plan_library_move(
    space: &Space<'_>,
    rel: &str,
    to_dir: &str,
) -> Result<LibraryMove, EntryError> {
    let abs = space.safe_path(rel, &Disk)?;
    let dest_dir = space.safe_path(to_dir, &Disk)?;
    let Some(library) = library_of(space.home(), &abs) else {
        return Err(refused(format!(
            "{rel} is not in one of the account's libraries"
        )));
    };
    assert_library_writable(space, &abs, "move")?;
    if library_of(space.home(), &dest_dir).is_none_or(|d| d.name != library.name) {
        return Err(refused(format!(
            "a module stays in its library — this one is {}'s",
            library.name
        )));
    }
    let active = space.active()?;
    guard_mutable(space, active.as_deref(), &abs, "move")?;
    let fs = StagedFs::new(&[&library.root]).map_err(|e| refused(e.to_string()))?;
    let lib = MoveLibrary {
        name: &library.name,
        root: &resolve(&posix(&library.root)),
    };
    let plan = plan_module_move(&fs, lib, &posix(&abs), &posix(&dest_dir))
        .map_err(|e| refused(e.to_string()))?;
    Ok(LibraryMove {
        root: library.root.clone(),
        from_abs: abs,
        dest: real(&plan.dest),
        from: plan.from,
        to: plan.to,
    })
}

/// The route's body for a move that landed: the new path, the two refs, the files the repoint changed.
#[must_use]
pub fn move_answer(space: &Space<'_>, plan: &LibraryMove, files: &[String]) -> serde_json::Value {
    serde_json::json!({
        "path": to_rel(space, &plan.dest),
        "from": plan.from,
        "to": plan.to,
        "repointed": files.iter().map(|f| to_rel(space, f)).collect::<Vec<_>>(),
    })
}
