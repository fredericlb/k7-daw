//! module_reads.rs — the module reads of a workspace: a module's panel, its engines and bases, the console systems (`workspaceModules.ts`, `engines.ts`).
//! What a reader gets wrong: nothing is written, so the stage mounts for reading only; a failure is the sentence a 400 carries.

use k7x::edit::{PathSeg, StringifyOptions, Value, stringify};
use serde_json::{Value as Json, json};
use song::modules::panels::module_ref_of;
use song::{Kind, Source};
use song_edit::base_preset::{BaseOwner, module_base_choices};
use song_edit::confine::Env;
use song_edit::loader::{load_module_refs, load_modules};
use song_edit::module_fork::shared_module_ref;
use song_edit::module_io::module_file_for;
use song_edit::paths;
use song_edit::workspace_doc::{ws_data, ws_text};
use song_edit::workspace_file::ws_exists;

use crate::confine::Disk;
use crate::edits::{StagedFs, env_for, env_of, posix};
use crate::libraries::Home;
use crate::rel::to_rel;
use crate::space::{Declined, PathError, Space};

/// The panel a module keeps beside its file.
const UI_FILE: &str = "ui.k7x";
/// The folder of a module that holds its engines.
const ENGINES_DIR: &str = "engines";
/// The engine a module runs when it names none and has one called this.
const DEFAULT_ENGINE: &str = "tracktion";
/// What the TypeScript says when a route needs the open project and none is.
pub const NO_ACTIVE: &str = "no active project — select a project in the workspace first";

/// Why a read has no answer: a sentence for a 400, or the reason the route is declined.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Say(String),
    Declined(Declined),
}

impl From<PathError> for Failure {
    fn from(e: PathError) -> Failure {
        match e {
            PathError::Declined(d) => Failure::Declined(d),
            e => Failure::Say(e.to_string()),
        }
    }
}

fn say<T>(s: impl Into<String>) -> Result<T, Failure> {
    Err(Failure::Say(s.into()))
}

/// The workspace a read runs in: its home, its code, its root and the open project the caller knows.
pub struct Reading<'a> {
    pub home: &'a Home,
    pub code_root: &'a str,
    pub root: &'a str,
    pub active: Option<&'a str>,
}

impl Reading<'_> {
    /// A read-only stage over what a project sees: the workspace, the libraries, the account's, the meta modules.
    pub(crate) fn stage(&self) -> Result<(StagedFs, Env), Failure> {
        let project = self.active.unwrap_or(self.root);
        let env = if self.active.is_some() {
            env_for(self.home, self.code_root, project)
        } else {
            env_of(self.home, self.code_root)
        };
        let mut reads = vec![
            env.meta_root.clone(),
            self.home.libraries.clone(),
            self.home.account_home(),
            self.root.to_owned(),
            project.to_owned(),
        ];
        reads.extend(env.overlays.iter().map(|(_, root)| root.clone()));
        let fs =
            StagedFs::reading::<&str, _>(&[], &reads).map_err(|e| Failure::Say(e.to_string()))?;
        Ok((fs, env))
    }

    fn require_active(&self) -> Result<&str, Failure> {
        self.active
            .ok_or_else(|| Failure::Say(NO_ACTIVE.to_owned()))
    }

    fn safe(&self, space: &Space<'_>, rel: &str) -> Result<String, Failure> {
        Ok(space.safe_path(rel, &Disk)?)
    }
}

/// `localModuleRef`: the ref of `…/modules/<type>/<name>/<file>` under the project, else none.
fn local_module_ref(dir: &str, abs: &str) -> Option<String> {
    let rel = crate::paths::relative(dir, abs);
    let segs: Vec<&str> = rel.split('/').collect();
    if segs.len() < 4 || segs[0] != "modules" {
        return None;
    }
    Some(module_ref_of(segs[1], &segs[2..segs.len() - 1].join("/")))
}

/// `uiFileText` of an empty panel: four columns, two rows, no widget, its header naming the module.
fn scaffold(reference: &str) -> Result<String, Failure> {
    let body = Value::Map(vec![
        ("cols".to_owned(), Value::Num(4.0)),
        ("rows".to_owned(), Value::Num(2.0)),
        ("widgets".to_owned(), Value::Map(vec![])),
    ]);
    let header = [format!("panel for {reference}")];
    let inline = |p: &[PathSeg], _: &Value| {
        p.len() == 2 && matches!(&p[0], PathSeg::Key(k) if k == "widgets")
    };
    stringify(
        &body,
        &StringifyOptions {
            header: &header,
            inline: Some(&inline),
        },
    )
    .map_err(|e| Failure::Say(e.to_string()))
}

/// `GET /module-ui`: the panel beside a module file, or the path it would take and a scaffold.
///
/// # Errors
/// A path outside the workspace, no open project, a file that is no module's, or a panel that will not read.
pub fn module_ui(r: &Reading<'_>, rel: &str) -> Result<Json, Failure> {
    let space = Space::open(r.home, r.root);
    let abs = r.safe(&space, rel)?;
    let dir = r.require_active()?;
    let (fs, env) = r.stage()?;
    let reference = shared_module_ref(&fs, &env, &posix(dir), &posix(&abs))
        .map(|s| s.reference)
        .or_else(|| local_module_ref(&posix(dir), &posix(&abs)));
    let Some(reference) = reference else {
        return say(format!("{} is not a module file", to_rel(&space, &abs)));
    };
    let path = paths::join(&[&paths::dirname(&posix(&abs)), UI_FILE]);
    let exists = ws_exists(&fs, &path);
    let content = if exists {
        ws_text(&fs, &path).map_err(|e| Failure::Say(e.to_string()))?
    } else {
        scaffold(&reference)?
    };
    let shown = to_rel(&space, &crate::edits::real(&path));
    Ok(json!({ "ref": reference, "path": shown, "content": content, "exists": exists }))
}

/// `GET /module-base`: the named bases a module may be set to, its ancestors' included; none when the vocabulary will not load.
///
/// # Errors
/// A path outside the workspace.
pub fn module_bases(r: &Reading<'_>, rel: &str) -> Result<Vec<String>, Failure> {
    let space = Space::open(r.home, r.root);
    let abs = r.safe(&space, rel)?;
    let dir = posix(r.active.unwrap_or(r.root));
    let (fs, env) = r.stage()?;
    let Ok(refs) = load_module_refs(&fs, &env, &dir) else {
        return Ok(vec![]);
    };
    let folder = paths::dirname(&posix(&abs));
    let own = refs
        .modules
        .values()
        .find(|m| m.folder_abs.as_deref() == Some(folder.as_str()));
    let owner = BaseOwner {
        extends: own.and_then(|m| m.authored.extends.clone()),
        folder_abs: Some(folder),
    };
    let lookup = |n: &str| {
        refs.modules.get(n).map(|m| BaseOwner {
            extends: m.authored.extends.clone(),
            folder_abs: m.folder_abs.clone(),
        })
    };
    Ok(module_base_choices(&fs, Some(&owner), &lookup))
}

/// Where each node's params come from: the modules it extends in order, the params it sets itself and the module that gave each other one.
///
/// # Errors
/// A workspace that will not stage, or a vocabulary that will not load.
pub fn node_origins(r: &Reading<'_>) -> Result<Json, Failure> {
    let dir = posix(r.active.unwrap_or(r.root));
    let (fs, env) = r.stage()?;
    let say = |e: song_edit::Error| Failure::Say(e.to_string());
    let (nodes, refs) = (
        load_modules(&fs, &env, &dir).map_err(say)?,
        load_module_refs(&fs, &env, &dir).map_err(say)?,
    );
    let keys = |m: &song::modules::Module| -> Vec<String> {
        m.params.iter().flatten().map(|(k, _)| k.clone()).collect()
    };
    let mut out = serde_json::Map::new();
    for (name, node) in &nodes {
        let (mut chain, mut from) = (Vec::<String>::new(), serde_json::Map::new());
        let mut next = node.authored.extends.clone();
        // The depth of an `extends` chain has no limit, a cycle is cut where it comes back.
        while let Some(reference) = next.filter(|r| !chain.contains(r)) {
            let module = refs.modules.get(&reference);
            for k in module.map(|m| keys(&m.authored)).unwrap_or_default() {
                from.entry(k).or_insert_with(|| json!(reference));
            }
            next = module.and_then(|m| m.authored.extends.clone());
            chain.push(reference);
        }
        out.insert(
            name.clone(),
            json!({ "extends": chain, "own": keys(&node.authored), "from": from }),
        );
    }
    Ok(Json::Object(out))
}

/// `GET /consoles`: the console systems the libraries hold, and the one the open project runs.
///
/// # Errors
/// No open project, or a vocabulary that will not load.
pub fn consoles(r: &Reading<'_>) -> Result<Json, Failure> {
    let dir = posix(r.require_active()?);
    let (fs, env) = r.stage()?;
    let current = ws_data(&fs, &paths::join(&[&dir, "project.k7x"]))
        .ok()
        .and_then(|v| match v {
            Value::Map(m) => m.into_iter().find(|(k, _)| k == "project").map(|(_, v)| v),
            _ => None,
        })
        .and_then(|v| match v {
            Value::Map(m) => m.into_iter().find(|(k, _)| k == "console").map(|(_, v)| v),
            _ => None,
        })
        .and_then(|v| match v {
            Value::Str(s) => Some(s),
            _ => None,
        });
    let vocab = load_module_refs(&fs, &env, &dir).map_err(|e| Failure::Say(e.to_string()))?;
    let systems = song::console::console_systems(&vocab);
    let mut out = json!({ "systems": systems });
    if let Some(c) = current {
        out["current"] = Json::String(c);
    }
    Ok(out)
}

/// `engineFoldersOf`: each sub-folder of `engines/` holding a module file, by name.
fn engine_folders(fs: &StagedFs, folder: &str) -> Vec<String> {
    let dir = paths::join(&[folder, ENGINES_DIR]);
    let Ok(entries) = fs.list(&dir) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .into_iter()
        .map(|e| e.name)
        .filter(|n| {
            !n.is_empty()
                && n.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .filter(|n| {
            fs.stat(&paths::join(&[&dir, n]))
                .is_ok_and(|s| s.kind == Kind::Dir)
        })
        .filter(|n| {
            module_file_for(
                fs,
                &paths::join(&[&dir, n]),
                &format!("{ENGINES_DIR}/{n}"),
                "module",
            )
            .is_some()
        })
        .collect();
    names.sort();
    names
}

/// `GET /module-engines`: the engines beside a module file and the one it names.
///
/// # Errors
/// A path outside the workspace.
pub fn module_engines(r: &Reading<'_>, rel: &str) -> Result<Json, Failure> {
    let space = Space::open(r.home, r.root);
    let abs = posix(&r.safe(&space, rel)?);
    let (fs, _) = r.stage()?;
    let mut names = engine_folders(&fs, &paths::dirname(&abs));
    if let Some(at) = names.iter().position(|n| n == DEFAULT_ENGINE) {
        let default = names.remove(at);
        names.insert(0, default);
    }
    let named = ws_data(&fs, &abs).ok().and_then(|v| match v {
        Value::Map(m) => m.into_iter().find(|(k, _)| k == "engine").map(|(_, v)| v),
        _ => None,
    });
    let current = match named {
        Some(Value::Str(w)) if names.contains(&w) => Some(w),
        _ => names.first().cloned(),
    };
    let mut out = json!({ "engines": names });
    if let Some(c) = current {
        out["engine"] = Json::String(c);
    }
    Ok(out)
}
