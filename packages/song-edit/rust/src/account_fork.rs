// account_fork.rs — FORK a module of the origin's libraries into the account's own: an `@account/<path>` module that `extends` it, with its panel.
// A reader would get wrong that the library is never written: changing a module of it is forking it, and the fork keeps inheriting what it leaves alone.

use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::library_forks::library_ref_parts;
use crate::module_move::repoint_refs;
use crate::paths::join;
use crate::shared_folders::{ACCOUNT_LIBRARY, Family, account_library_home, library_family_root};
use crate::workspace_doc::ws_write_text;
use k7x::edit::{StringifyOptions, Value, stringify};

const MODULE_FILE: &str = "module.k7x";
const UI_FILE: &str = "ui.k7x";

/// What a fork did: the new ref, its folder, and the project files that now name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountFork {
    pub reference: String,
    pub folder: String,
    pub created: bool,
    pub repointed: Vec<String>,
}

/// `path` split on `/`, or None when any segment is empty, `.` or `..`.
fn plain_segments(path: &str) -> Option<Vec<&str>> {
    let segs: Vec<&str> = path.split('/').collect();
    segs.iter()
        .all(|s| !s.is_empty() && *s != "." && *s != "..")
        .then_some(segs)
}

/// Fork the library module `r` (as `project_dir`'s workspace pins it) into `@account`, and repoint the project to it.
///
/// # Errors
/// A ref that names no module of the origin's libraries, or the file system's.
pub fn fork_into_account(fs: &mut dyn Fs, env: &Env, project_dir: &str, r: &str) -> Res<AccountFork> {
    let Some((library, rest)) = library_ref_parts(r).filter(|(l, _)| *l != ACCOUNT_LIBRARY) else {
        return fail(format!("{r} is no module of the origin's libraries"));
    };
    // A ref may name one node of the module (`…/x:warm`); the fork is of the module.
    let path = rest.split(':').next().unwrap_or(rest);
    let Some(segs) = plain_segments(path) else {
        return fail(format!("{r} is no module path"));
    };
    let modules = library_family_root(fs, env, library, Family::Modules, Some(project_dir));
    let base = modules.map(|m| join(&[&m, &segs.join("/")]));
    let Some(base) = base.filter(|b| fs.exists(&join(&[b, MODULE_FILE]))) else {
        return fail(format!("{r}: no such module in the version this workspace plays"));
    };
    let base_ref = format!("@{library}/{path}");
    let fork_ref = format!("@{ACCOUNT_LIBRARY}/{path}");
    let folder = join(&[&account_library_home(env), "modules", &segs.join("/")]);
    // Already forked: the project is pointed at it, and what the fork holds is left alone.
    let created = !fs.exists(&join(&[&folder, MODULE_FILE]));
    if created {
        fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
        let body = Value::object(vec![("extends".to_owned(), Value::Str(base_ref.clone()))]);
        ws_write_text(
            fs,
            &join(&[&folder, MODULE_FILE]),
            &stringify(&body, &StringifyOptions::default())?,
        )?;
        let ui = join(&[&base, UI_FILE]);
        if fs.exists(&ui) {
            fs.copy(&ui, &join(&[&folder, UI_FILE]))
                .map_err(|e| io_error(&e, "copyfile", &ui))?;
        }
    }
    let repointed = repoint_refs(fs, &[project_dir.to_owned()], &base_ref, &fork_ref, false)?;
    Ok(AccountFork {
        reference: fork_ref,
        folder,
        created,
        repointed,
    })
}
