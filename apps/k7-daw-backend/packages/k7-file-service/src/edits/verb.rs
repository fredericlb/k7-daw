//! verb.rs — one command line run by the `verbs` crate over a `StagedFs` of the workspace, its writes landed whole (docs/verbs-port.md).
//! What a reader gets wrong: a verb that refuses, throws or waits for a route lands nothing, even the writes it made before; the caller holds the lock.

use song::Cache;
use song_edit::confine::Env;
use song_edit::ids::NewId;
pub use verbs::Replay;
use verbs::{Answer, Item, complete_in_env, run_replayed};

use super::{StagedFs, env_for, env_of, posix};
use crate::atomic::Real;
use crate::changes::commit::{Landed, commit};
use crate::changes::plan::Act;
use crate::changes::target::{Target, confined};
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
    /// What the line's earlier runs fixed: the routes' answers and the stamp.
    pub replay: Replay,
    /// The project is a library chain folder: the line may write only there and the shared texts.
    pub chain: bool,
}

/// The folder the line runs on; with no project, a folder of the workspace that is never there, so the workspace's own files are found beside it.
fn project_dir(at: &Line<'_>) -> String {
    at.project.map_or_else(
        || posix(&crate::paths::join(&[at.root, ".no-project"])),
        posix,
    )
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
    let dir = project_dir(at);
    let answer = run_replayed(
        &mut fs,
        cache,
        new_id,
        env,
        &dir,
        at.line,
        at.replay.clone(),
    );
    if answer.refused || answer.pending.is_some() || fs.is_empty() {
        return Ok(Ran {
            answer,
            ..Ran::default()
        });
    }
    let set = fs.planned()?;
    if at.chain {
        let chain = Target::Chain(dir.clone());
        confined(&chain, at.root, &set).map_err(|r| format!("{r:?}"))?;
    }
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

/// The line up to `cursor` UTF-16 units, as a browser's caret counts them; absent, the whole line.
fn head_of(line: &str, cursor: Option<usize>) -> &str {
    let Some(cursor) = cursor else {
        return line;
    };
    let mut units = 0;
    for (i, c) in line.char_indices() {
        if units >= cursor {
            return &line[..i];
        }
        units += c.len_utf16();
    }
    line
}

/// The completions of `at.line` cut at `cursor`, read over the workspace and written nowhere.
///
/// # Errors
/// A mount that is not absolute.
pub fn complete_verb(
    at: &Line<'_>,
    cursor: Option<usize>,
    cache: &mut Cache,
) -> Result<Vec<Item>, String> {
    let env = at.project.map_or_else(
        || env_of(at.home, at.code_root),
        |p| env_for(at.home, at.code_root, p),
    );
    let (writes, reads) = mounts(at.home, at.root, &env);
    let fs = StagedFs::reading(&writes, &reads).map_err(|e| e.to_string())?;
    let dir = project_dir(at);
    Ok(complete_in_env(
        &fs,
        cache,
        env,
        &dir,
        head_of(at.line, cursor),
    ))
}

#[cfg(test)]
mod tests {
    use super::head_of;

    #[test]
    fn the_caret_counts_utf16_units() {
        assert_eq!(head_of("track rename é x", Some(14)), "track rename é");
        assert_eq!(head_of("a😀b", Some(3)), "a😀");
        assert_eq!(head_of("ab", None), "ab");
        assert_eq!(head_of("ab", Some(9)), "ab");
    }
}
