//! verb.rs — one command line run by the `verbs` crate over a `StagedFs` of the workspace, its writes landed whole (docs/verbs-port.md).
//! What a reader gets wrong: a verb that refuses or throws lands nothing, even the writes it made before it said no; the caller holds the lock.

use song::Cache;
use song_edit::confine::Env;
use song_edit::ids::NewId;
use verbs::{Answer, run_in_env};

use super::{StagedFs, env_for, env_of, posix};
use crate::atomic::Real;
use crate::changes::commit::{Landed, commit};
use crate::changes::plan::Act;
use crate::libraries::Home;

/// What a line came to: the verb's answer, what landed (none for a refusal or no write), and the texts it wrote.
#[derive(Debug, Default)]
pub struct Ran {
    pub answer: Answer,
    pub landed: Option<Landed>,
    /// Every text the landing wrote, absolute: the stamp and the relay read them.
    pub written: Vec<(String, String)>,
}

/// The line and where it runs: the account's home, the workspace, the code's root and the open project (none: a project-less verb).
pub struct Line<'a> {
    pub home: &'a Home,
    pub root: &'a str,
    pub code_root: &'a str,
    pub project: Option<&'a str>,
    pub line: &'a str,
}

/// Where a verb may write: the workspace (its project and the shared texts) and the studio's own folder (the preferences).
fn mounts(home: &Home, root: &str, env: &Env) -> (Vec<String>, Vec<String>) {
    let writes = vec![posix(root), posix(&home.studio)];
    let mut reads = vec![
        env.meta_root.clone(),
        env.libraries_home.clone(),
        posix(&home.account_home()),
    ];
    reads.extend(env.overlays.iter().map(|(_, r)| r.clone()));
    if let Some(roots) = &env.library_roots {
        reads.extend(roots.iter().map(|(_, r)| r.clone()));
    }
    (writes, reads)
}

/// Runs the line, landing its writes; `snapshot` is asked for each binary the landing replaces.
///
/// # Errors
/// A mount that is not absolute, a staged file that is not text, or the landing's failure; nothing is written then.
pub fn run_verb(
    at: &Line<'_>,
    cache: &mut Cache,
    new_id: NewId,
    snapshot: &mut dyn FnMut(&str),
) -> Result<Ran, String> {
    let env = at.project.map_or_else(
        || env_of(at.home, at.code_root),
        |p| env_for(at.home, at.code_root, p),
    );
    let (writes, reads) = mounts(at.home, at.root, &env);
    let mut fs = StagedFs::reading(&writes, &reads).map_err(|e| e.to_string())?;
    let dir = at.project.map(posix).unwrap_or_default();
    let answer = run_in_env(&mut fs, cache, new_id, env, &dir, at.line);
    if answer.refused || fs.is_empty() {
        return Ok(Ran {
            answer,
            ..Ran::default()
        });
    }
    let set = fs.planned()?;
    let landed = commit(&Real, &set, snapshot).map_err(|e| e.what)?;
    fs.drop_emptied_dirs();
    let written = set
        .acts
        .iter()
        .filter_map(|a| match a {
            Act::Text { abs, text, .. } => Some((abs.clone(), text.clone())),
            _ => None,
        })
        .collect();
    Ok(Ran {
        answer,
        landed: Some(landed),
        written,
    })
}
