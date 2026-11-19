//! open.rs — the repair of an open (the picture strips) and its seconds stamp, the repair's staged set landed whole (C10).
//! What a reader gets wrong: a repair that fails lands nothing and the stamp still runs; the caller holds the account's write lock.

use song_edit::ids::NewId;
use song_edit::open_repairs::{Note, repair_picture_strips};

use super::stamp::stamp_format_at_open;
use super::{StagedFs, env_of};
use crate::atomic::Real;
use crate::changes::commit::commit;
use crate::entries::guards::assert_library_writable;
use crate::libraries::Home;
use crate::space::Space;

/// What the open's repairs said and whether they changed the project's files.
#[derive(Debug, Default)]
pub struct Repaired {
    pub notes: Vec<Note>,
    pub touched: bool,
    pub structural: bool,
    /// Why nothing was repaired: the project is in a library this account cannot write.
    pub skipped: Option<String>,
}

/// Every repair on `project` (named `rel` in what they say), then the seconds stamp; the notes are for the log.
/// A project of a library the account cannot write gets neither, and `skipped` says why.
pub fn repair_open(
    home: &Home,
    root: &str,
    code_root: &str,
    project: &str,
    rel: &str,
    new_id: NewId,
) -> Repaired {
    // A library project the account cannot write, or a published version, is left as it is.
    if let Err(e) = assert_library_writable(&Space::open(home, root), project, "repair") {
        return Repaired {
            skipped: Some(e.to_string()),
            ..Repaired::default()
        };
    }
    let env = env_of(home, code_root);
    let mut reads = vec![
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
    ];
    reads.extend(env.overlays.iter().map(|(_, root)| root.clone()));
    let mut out = Repaired::default();
    let staged = StagedFs::reading(&[project], &reads).map_err(|e| e.to_string()).and_then(|mut fs| {
        let notes = repair_picture_strips(&mut fs, &env, project, rel, &mut *new_id)?;
        let landed = fs
            .planned()
            .and_then(|set| commit(&Real, &set, &mut |_| {}).map_err(|e| e.what))?;
        fs.drop_emptied_dirs();
        out.touched |= !landed.touched.is_empty();
        out.structural |= landed.structural;
        Ok(notes)
    });
    // A repair that fails is silent: the project is left as it was.
    out.notes.extend(staged.unwrap_or_default());
    if stamp_format_at_open(project) {
        out.touched = true;
    }
    out
}
