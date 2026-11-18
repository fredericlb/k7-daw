//! module_make.rs — an empty module made in one of the libraries the workspace lists (`createModuleIn`), staged and landed whole.
//! What a reader gets wrong: the root must be one the listing answers, never a folder the caller named; the caller holds the lock.

use indexmap::IndexMap;
use k7x::edit::{StringifyOptions, Value, stringify};
use serde_json::{Value as Json, json};
use song::modules::LoadedModule;
use song_edit::paths::{join, resolve};
use song_edit::project_files::{clone_module_file, create_node, free_node_name};
use song_edit::shared_folders::{account_libraries, module_library_dirs};
use song_edit::workspace_file::ws_exists;

use super::{StagedFs, env_of, posix, real};
use crate::atomic::Real;
use crate::changes::commit::{Landed, commit};
use crate::confine::Disk;
use crate::entries::EntryError;
use crate::entries::guards::assert_library_writable;
use crate::libraries::{Home, library_of};
use crate::rel::to_rel;
use crate::space::Space;

fn refused(s: String) -> EntryError {
    EntryError::Refused(s)
}

/// `/^[A-Za-z0-9][A-Za-z0-9._ -]*$/`: a folder's name, as the route allows one.
fn is_folder_name(seg: &str) -> bool {
    let mut chars = seg.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ' ' | '-'))
}

/// The folders a module library is listed at: `moduleRootsOf` plus the workspace's `_modules`.
fn listed_roots(
    fs: &StagedFs,
    env: &song_edit::confine::Env,
    dir: &str,
    root: &str,
) -> Vec<String> {
    let mut roots = module_library_dirs(fs, env, dir);
    // Every library is a row, one with no `modules/` yet included.
    for lib in account_libraries(fs, env, Some(dir)) {
        let at = join(&[&lib.root, "modules"]);
        if !roots.contains(&at) && !roots.iter().any(|r| resolve(r) == resolve(&at)) {
            roots.push(at);
        }
    }
    roots.push(join(&[root, "_modules"]));
    roots.iter().map(|r| posix(r)).collect()
}

/// What a module made came to: the answer and what the landing touched.
pub struct Made {
    pub answer: Json,
    pub landed: Landed,
}

/// `createModuleIn`'s judging, in its order, then the one landing.
///
/// # Errors
/// The route's refusals, or the landing's failure.
pub fn create_module(
    home: &Home,
    code_root: &str,
    space: &Space<'_>,
    root: &str,
    rel: &str,
) -> Result<Made, EntryError> {
    let segs: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return Err(refused(format!(
            "a module is named <type>/<name> — '{rel}' names no type folder"
        )));
    }
    if let Some(bad) = segs.iter().find(|s| !is_folder_name(s)) {
        return Err(refused(format!(
            "'{bad}' is not something a folder can be called"
        )));
    }
    let root_abs = space.safe_path(root, &Disk)?;
    let env = env_of(home, code_root);
    let active = space.active()?;
    let from = active.clone().unwrap_or_else(|| space.root().to_string());
    let reads = [
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
    ];
    let fs = StagedFs::reading(&[space.root()], &reads).map_err(|e| refused(e.to_string()))?;
    let listed = listed_roots(&fs, &env, &posix(&from), &posix(space.root()));
    if !listed.contains(&posix(&root_abs)) {
        return Err(refused(format!(
            "'{root}' is not a module library of this workspace"
        )));
    }
    let dir = space.safe_path(&format!("{root}/{}", segs.join("/")), &Disk)?;
    assert_library_writable(space, &dir, "create a module in")?;
    let file = join(&[&dir, "module.k7x"]);
    let mount = library_of(home, &dir).map_or_else(|| space.root().to_string(), |l| l.root);
    let mut fs = StagedFs::reading(&[mount], &reads).map_err(|e| refused(e.to_string()))?;
    if ws_exists(&fs, &posix(&file)) {
        return Err(refused(format!("{} already exists", to_rel(space, &file))));
    }
    let blank = Value::object(vec![(
        "plugin".to_owned(),
        Value::object(vec![
            ("type".to_owned(), Value::Null),
            ("name".to_owned(), Value::Null),
        ]),
    )]);
    let header = [segs[segs.len() - 1].to_string()];
    let text = stringify(
        &blank,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )
    .map_err(|e| refused(e.to_string()))?;
    song_edit::fs::Fs::mkdir_all(&mut fs, &posix(&dir)).map_err(|e| refused(e.to_string()))?;
    song_edit::fs::Fs::write_atomic(&mut fs, &posix(&file), text.as_bytes())
        .map_err(|e| refused(e.to_string()))?;
    let set = fs.planned().map_err(refused)?;
    let landed = commit(&Real, &set, &mut |_| {}).map_err(|e| refused(e.what))?;
    let answer = json!({ "path": to_rel(space, &real(&file)), "dir": to_rel(space, &real(&dir)) });
    Ok(Made { answer, landed })
}

const NO_ACTIVE: &str = "no active project — select a project in the workspace first";

/// A stage that reads what a node or a clone needs and writes inside the workspace only.
fn workspace_stage(
    home: &Home,
    code_root: &str,
    space: &Space<'_>,
) -> Result<StagedFs, EntryError> {
    let env = env_of(home, code_root);
    let reads = [
        env.meta_root.clone(),
        home.libraries.clone(),
        home.account_home(),
    ];
    StagedFs::reading(&[space.root()], &reads).map_err(|e| refused(e.to_string()))
}

fn landed(fs: &StagedFs) -> Result<Landed, EntryError> {
    let set = fs.planned().map_err(refused)?;
    commit(&Real, &set, &mut |_| {}).map_err(|e| refused(e.what))
}

/// The module folder `abs` names: itself, or the folder of a file inside it; `(folder, category, name)`.
fn module_of(abs: &str) -> (String, String, String) {
    let name_of = |p: &str| song_edit::paths::basename(p).to_owned();
    let dir = if song_edit::sugar::is_doc_file(&name_of(abs)) {
        song_edit::paths::dirname(abs)
    } else {
        abs.to_owned()
    };
    let category = name_of(&song_edit::paths::dirname(&dir));
    let name = name_of(&dir);
    (dir, category, name)
}

/// `cloneModuleIn`: the module of `rel` copied under `new_name` beside the open project.
///
/// # Errors
/// No open project, or any refusal of the clone.
pub fn clone_module(
    _home: &Home,
    _code_root: &str,
    space: &Space<'_>,
    rel: &str,
    new_name: &str,
) -> Result<Made, EntryError> {
    let abs = space.safe_path(rel, &Disk)?;
    let (_, category, name) = module_of(&posix(&abs));
    let dir = space.active()?.ok_or_else(|| refused(NO_ACTIVE.into()))?;
    // The clone reads its source inside the workspace alone: no library is mounted for a link to lead into.
    let mut fs = StagedFs::new(&[space.root()]).map_err(|e| refused(e.to_string()))?;
    let made = clone_module_file(&mut fs, &posix(&dir), &category, &name, new_name)
        .map_err(|e| refused(e.to_string()))?;
    let at = song_edit::paths::resolve(&join(&[&posix(&dir), &made]));
    let landed = landed(&fs)?;
    Ok(Made {
        answer: json!({ "path": to_rel(space, &real(&at)) }),
        landed,
    })
}

/// `createNodeIn`: one use of the module of `rel` in the open project; the name is `node` or the first free one.
///
/// # Errors
/// No open project, or any refusal of the node.
pub fn create_node_of(
    home: &Home,
    code_root: &str,
    space: &Space<'_>,
    rel: &str,
    node: Option<&str>,
    catalogue: &IndexMap<String, LoadedModule>,
    new_id: song_edit::ids::NewId,
) -> Result<Made, EntryError> {
    let abs = space.safe_path(rel, &Disk)?;
    let (_, category, name) = module_of(&posix(&abs));
    let dir = posix(&space.active()?.ok_or_else(|| refused(NO_ACTIVE.into()))?);
    let env = env_of(home, code_root);
    let mut fs = workspace_stage(home, code_root, space)?;
    let node = match node.filter(|n| !n.is_empty()) {
        Some(n) => n.to_owned(),
        None => free_node_name(&fs, &env, &dir, &name, None),
    };
    let path = create_node(
        &mut fs, &env, &dir, &category, &name, &node, catalogue, new_id,
    )
    .map_err(|e| refused(e.to_string()))?;
    let landed = landed(&fs)?;
    Ok(Made {
        answer: json!({ "path": path }),
        landed,
    })
}
