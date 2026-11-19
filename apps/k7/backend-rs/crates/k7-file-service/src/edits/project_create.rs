//! project_create.rs — a project made from its archetype (`createProjectIn`): scaffolded in memory, landed whole, then its owed modules named.
//! What a reader gets wrong: the sample video and the workspace's shared synth are written beside the project, so the workspace is the mount.

use serde_json::json;
use song_edit::ids::NewId;
use song_edit::loader::load_module_refs;
use song_edit::scaffold::{TimelineShape, project_files, scaffold, video_project_files};
use song_edit::scaffold_live::{
    live_project_files, live_video_project_files, scaffold_live, scaffold_live_video,
};
use song_edit::scaffold_needs::{missing_refs, needed_refs};
use song_edit::scaffold_set::{scaffold_set, set_project_files};

use super::module_make::Made;
use super::{StagedFs, env_for, posix};
use crate::atomic::Real;
use crate::changes::commit::commit;
use crate::confine::Disk;
use crate::entries::EntryError;
use crate::filter::project_app;
use crate::libraries::Home;
use crate::paths::basename;
use crate::rel::to_rel;
use crate::space::Space;
use crate::workspaces::EXAMPLES;

fn refused(s: impl Into<String>) -> EntryError {
    EntryError::Refused(s.into())
}

/// The name a project is CALLED: the folder's with its `.k7` taken off.
fn label(name: &str) -> &str {
    if project_app(name).is_some() {
        &name[..name.len() - ".k7".len()]
    } else {
        name
    }
}

/// The folder name a typed one becomes: separators and leading dots are not kept.
fn bare_of(raw: &str) -> String {
    let flat = raw.trim().replace(['/', '\\'], "_");
    flat.trim_start_matches('.').to_owned()
}

/// What each archetype's file map is, without writing it, so what it will NEED can be read first.
fn files_of(shape: &str, name: &str) -> Vec<(String, String)> {
    // The ids are minted for nothing: only the files' text is read.
    let mut spare = || String::new();
    match shape {
        "arrangement" => project_files(name, &mut spare),
        "video" => video_project_files(name, &mut spare),
        "mix" => set_project_files(label(name)),
        "live" => live_project_files(label(name)),
        _ => live_video_project_files(label(name)),
    }
}

/// `createProjectIn`: the answer is `{project, missing}`.
///
/// # Errors
/// The sentences of the TypeScript's refusals, the sample video's absence, or the landing's failure.
pub fn create_project(
    home: &Home,
    code_root: &str,
    space: &Space<'_>,
    shape: &str,
    raw_name: &str,
    new_id: NewId,
) -> Result<Made, EntryError> {
    if basename(space.root()) == EXAMPLES {
        return Err(refused(format!(
            "{EXAMPLES} holds the bundled demos and takes no new project — make it in a workspace of your own"
        )));
    }
    let bare = bare_of(raw_name);
    if bare.is_empty() {
        return Err(refused("project name required"));
    }
    let name = if project_app(&bare).is_some() {
        bare
    } else {
        format!("{bare}.k7")
    };
    let abs = space.safe_path(&name, &Disk)?;
    if abs == space.root() {
        return Err(refused("project name required"));
    }
    if std::fs::metadata(&abs).is_ok_and(|m| m.is_file()) {
        return Err(refused(format!(
            "ENOTDIR: not a directory, scandir '{abs}'"
        )));
    }
    if std::fs::read_dir(&abs).is_ok_and(|mut d| d.next().is_some()) {
        return Err(refused(format!(
            "\"{name}\" already exists and is not empty"
        )));
    }
    let env = env_for(home, code_root, &abs);
    let reads = [
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
    ];
    let mut fs = StagedFs::reading(&[space.root()], &reads).map_err(|e| refused(e.to_string()))?;
    let dir = posix(&abs);
    let sample_bytes = if shape == "video" {
        Some(sample_video(code_root)?)
    } else {
        None
    };
    let made = match shape {
        "arrangement" | "video" => {
            let picture = shape == "video";
            let kind = if picture {
                TimelineShape::Video
            } else {
                TimelineShape::Arrangement
            };
            // A staged file is text: the sample goes in empty and its bytes replace it once the set has landed.
            scaffold(
                &mut fs,
                &dir,
                &name,
                kind,
                sample_bytes.as_ref().map(|_| &[][..]),
                new_id,
            )
        }
        "mix" => scaffold_set(&mut fs, &dir, label(&name)),
        "live" => scaffold_live(&mut fs, &dir, label(&name), new_id),
        _ => scaffold_live_video(&mut fs, &dir, label(&name), new_id),
    };
    made.map_err(|e| refused(e.to_string()))?;
    let set = fs.planned().map_err(refused)?;
    let landed = commit(&Real, &set, &mut |_| {}).map_err(|e| refused(e.what))?;
    if let Some(bytes) = sample_bytes {
        let to = super::real(&posix(&format!("{abs}/clips/sample.mp4")));
        // The folder was absent or empty before: a failed sample takes the project back with it.
        if let Err(e) = crate::atomic::write_atomic(&to, &bytes) {
            let _ = std::fs::remove_dir_all(super::real(&dir));
            return Err(refused(e.to_string()));
        }
    }
    let missing = owed(home, code_root, space, &abs, &files_of(shape, &name));
    let answer = json!({ "project": to_rel(space, &abs), "missing": missing });
    Ok(Made { answer, landed })
}

/// The bundled sample the `video` archetype copies in.
fn sample_video(code_root: &str) -> Result<Vec<u8>, EntryError> {
    let at = format!("{code_root}/packages/song/samples/video/sample.mp4");
    std::fs::read(&at).map_err(|_| {
        refused(format!(
            "bundled sample video missing ({at}) — the @studio/song package is incomplete"
        ))
    })
}

/// The shared modules the new project names and its libraries lack; an unreadable ref map owes none.
fn owed(
    home: &Home,
    code_root: &str,
    space: &Space<'_>,
    abs: &str,
    files: &[(String, String)],
) -> Vec<String> {
    let env = env_for(home, code_root, abs);
    let reads = [
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
        space.root().to_owned(),
    ];
    let Ok(fs) = StagedFs::reading::<&str, _>(&[], &reads) else {
        return vec![];
    };
    let Ok(known) = load_module_refs(&fs, &env, &posix(abs)) else {
        return vec![];
    };
    missing_refs(&needed_refs(files), &known)
}
