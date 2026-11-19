//! copy.rs — a project copied from one workspace into another, planned and then written: both ends' guards, every
//! entry `cpSync` would write (links followed), the playlists its surfaces name and the `@workspace` modules it names.
//! What a reader gets wrong: the plan reads the source BEFORE a byte is written, so a link loop is declined, not looped.

use std::collections::HashSet;
use std::fs;
use std::io::Read;

use serde_json::json;

use super::names::{child_below, host_order, is_project_dir, name_ok, on_disk, workspace_dir};
use super::refs::{Todo, is_text_name, refs_in};
use super::surfaces::project_surfaces;
use super::{EXAMPLES, WsError};
use crate::confine::Disk;
use crate::entries::guards::is_recycled;
use crate::filter::{code_of, project_app, skip};
use crate::libraries::Home;
use crate::paths::{basename, dirname, is_within, join};
use crate::rel::to_rel;
use crate::space::{Declined, Space, js_trim};
use crate::tree::Limits;

/// The folder a workspace shares its modules from; a copy never writes outside it for a ref.
const MODULES_DIR: &str = "_modules";
/// Where a workspace keeps its playlists.
const PLAYLISTS_DIR: &str = "_playlists";
/// The extension of a workspace document; `DOC_EXT`'s dot is a wildcard in the TypeScript (see `is_doc_file`).
const DOC_EXT: &str = "k7x";

/// What a copy is asked: the names as the request carried them.
#[derive(Clone, Copy, Debug)]
pub struct CopyAsk<'a> {
    pub from: &'a str,
    pub path: &'a str,
    pub to: &'a str,
    pub name: &'a str,
}

/// One thing the copy writes, absolute, in the order it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Folder(String),
    File { from: String, to: String },
}

/// A copy judged and not done: what it writes, and the answer it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyPlan {
    pub src: String,
    pub dest: String,
    pub entries: Vec<Entry>,
    pub project: String,
    pub playlists: Vec<String>,
    pub modules: Vec<String>,
}

fn io(e: &std::io::Error, syscall: &'static str, path: &str) -> WsError {
    WsError::Io {
        code: code_of(e),
        syscall,
        path: path.into(),
    }
}

/// `rawName.trim().replace(/[/\\]/g, "_").replace(/^\.+/, "")`: the one segment a copy is named.
pub fn clean_name(raw: &str) -> String {
    js_trim(raw)
        .replace(['/', '\\'], "_")
        .trim_start_matches('.')
        .to_string()
}

/// `isDocFile`: `new RegExp(".k7x$", "i")`, whose dot takes any character but a line terminator.
pub fn is_doc_file(name: &str) -> bool {
    let n = name.len();
    let Some(tail) = name.get(n.saturating_sub(DOC_EXT.len())..) else {
        return false;
    };
    if n < DOC_EXT.len() + 1 || !tail.eq_ignore_ascii_case(DOC_EXT) {
        return false;
    }
    name[..n - DOC_EXT.len()]
        .chars()
        .next_back()
        .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
}

/// `playlistPathOf`: a playlist name as the workspace path holding it; none for one that would leave the folder.
pub fn playlist_path_of(name: &str) -> Option<String> {
    let clean = js_trim(name).trim_matches('/');
    if clean.is_empty()
        || clean
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return None;
    }
    Some(if is_doc_file(clean) {
        format!("{PLAYLISTS_DIR}/{clean}")
    } else {
        format!("{PLAYLISTS_DIR}/{clean}.{DOC_EXT}")
    })
}

/// The name a copy is made under: Examples takes none, a blank is refused, and the suffix stays the source's.
pub fn copy_name(dest_workspace: &str, src_base: &str, raw: &str) -> Result<String, WsError> {
    if dest_workspace == EXAMPLES {
        return Err(WsError::Refused(format!(
            "{EXAMPLES} holds the bundled demos and takes no new project — make it in a workspace of your own"
        )));
    }
    let name = clean_name(raw);
    if name.is_empty() {
        return Err(WsError::Refused("project name required".into()));
    }
    if project_app(&name).is_none() || project_app(&name) != project_app(src_base) {
        return Err(WsError::Refused(format!(
            "a copy of {src_base} must keep its suffix: \"{name}\" does not"
        )));
    }
    Ok(name)
}

/// A workspace named by the request, in the folder's own spelling (1632): its name judged, then its presence.
fn end(home: &Home, asked: &str) -> Result<(String, String), WsError> {
    if !name_ok(asked) {
        return Err(WsError::Invalid { name: asked.into() });
    }
    let dir = workspace_dir(home);
    let spelled = on_disk(&dir, asked).ok_or_else(|| WsError::Unknown { name: asked.into() })?;
    let root = join(&[&dir, &spelled]);
    Ok((spelled, root))
}

/// The entries `cpSync(from, to, { recursive })` writes, links followed, those `keep` refuses left out, counted against
/// the limits before anything is written; an entry that cannot be read fails the plan as it would fail the copy.
pub fn plan_tree(
    from: &str,
    to: &str,
    keep: &dyn Fn(&str) -> bool,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<Entry>, WsError> {
    let mut out = Vec::new();
    let meta = fs::metadata(from).map_err(|e| io(&e, "stat", from))?;
    if !meta.is_dir() {
        out.push(Entry::File {
            from: from.into(),
            to: to.into(),
        });
        return Ok(out);
    }
    out.push(Entry::Folder(to.into()));
    let list = |dir: &str, seen: &mut u64| -> Result<Vec<String>, WsError> {
        let mut names = Vec::new();
        for e in fs::read_dir(dir).map_err(|e| io(&e, "scandir", dir))? {
            let e = e.map_err(|e| io(&e, "scandir", dir))?;
            *seen += 1;
            if *seen > limits.max_seen {
                return Err(Declined::TooMany.into());
            }
            names.push(e.file_name().to_string_lossy().into_owned());
        }
        host_order(&mut names);
        Ok(names)
    };
    let mut stack = vec![(from.to_string(), to.to_string(), list(from, seen)?, 0usize)];
    while let Some((src, dst, names, at)) = stack.last_mut() {
        let Some(name) = names.get(*at).cloned() else {
            stack.pop();
            continue;
        };
        *at += 1;
        if !keep(&name) {
            continue;
        }
        // A `\`-spelled name folding out of its folder would read and write outside both trees (one file truncated onto itself).
        let (Some(a), Some(b)) = (child_below(src, &name), child_below(dst, &name)) else {
            continue;
        };
        if fs::metadata(&a).map_err(|e| io(&e, "stat", &a))?.is_dir() {
            if u32::try_from(stack.len()).unwrap_or(u32::MAX) > limits.max_depth {
                return Err(Declined::TooDeep.into());
            }
            out.push(Entry::Folder(b.clone()));
            let names = list(&a, seen)?;
            stack.push((a, b, names, 0));
        } else {
            out.push(Entry::File { from: a, to: b });
        }
    }
    Ok(out)
}

/// The disk's part: each folder made, each file copied over what is there (`copyFileSync`); a failure leaves what was written.
pub fn apply_entries(entries: &[Entry]) -> Result<(), WsError> {
    for e in entries {
        match e {
            Entry::Folder(p) => fs::create_dir_all(p).map_err(|e| io(&e, "mkdir", p))?,
            Entry::File { from, to } => {
                let parent = dirname(to);
                fs::create_dir_all(&parent).map_err(|e| io(&e, "mkdir", &parent))?;
                fs::copy(from, to).map_err(|e| io(&e, "copyfile", from))?;
            }
        }
    }
    Ok(())
}

/// `plan_tree` of a project: what `skip` hides is not content.
fn plan_project(
    src: &str,
    dest: &str,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<Entry>, WsError> {
    plan_tree(src, dest, &|n| !skip(n), limits, seen)
}

fn text_of(path: &str, limits: &Limits) -> Result<String, WsError> {
    let f = fs::File::open(path).map_err(|e| io(&e, "open", path))?;
    let mut bytes = Vec::new();
    f.take(limits.max_text + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io(&e, "read", path))?;
    if bytes.len() as u64 > limits.max_text {
        return Err(Declined::TooLarge.into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Whether a copied file under `base` is one `workspaceRefs` reads: a text name, in no `node_modules` or dot folder.
fn scanned(base: &str, file: &str) -> bool {
    let below = file.strip_prefix(base).unwrap_or(file);
    let mut segs: Vec<&str> = below.split('/').filter(|s| !s.is_empty()).collect();
    let name = segs.pop().unwrap_or("");
    is_text_name(name)
        && segs
            .iter()
            .all(|s| *s != "node_modules" && !s.starts_with('.'))
}

/// The module paths named in the text files a plan copies under `base`, in the order they are copied.
fn refs_of_entries(
    base: &str,
    entries: &[Entry],
    into: &mut Todo,
    limits: &Limits,
) -> Result<(), WsError> {
    for e in entries {
        if let Entry::File { from, to } = e
            && scanned(base, to)
        {
            refs_in(&text_of(from, limits)?, into);
        }
    }
    Ok(())
}

/// What the plan has written so far, so a later `existsSync` sees it as the TypeScript's would after the earlier writes;
/// on a disk that folds case (APFS, NTFS) `Fred.k7x` written is `fred.k7x` there.
struct Written {
    paths: HashSet<String>,
    fold: bool,
}

impl Written {
    /// Whether the disk folds case, asked of the workspaces' folder spelled in capitals.
    fn new(dest_root: &str) -> Written {
        let dir = dirname(dest_root);
        let upper = join(&[&dirname(&dir), &basename(&dir).to_ascii_uppercase()]);
        Written {
            paths: HashSet::new(),
            fold: upper != dir && fs::metadata(&upper).is_ok(),
        }
    }

    fn key(&self, p: &str) -> String {
        if self.fold {
            p.to_ascii_lowercase()
        } else {
            p.to_string()
        }
    }

    fn exists(&self, p: &str) -> bool {
        self.paths.contains(&self.key(p)) || fs::metadata(p).is_ok()
    }

    fn note(&mut self, e: &Entry) {
        let (Entry::Folder(p) | Entry::File { to: p, .. }) = e;
        let mut at = p.as_str().to_string();
        loop {
            if !self.paths.insert(self.key(&at)) {
                break;
            }
            let up = dirname(&at);
            if up == at {
                break;
            }
            at = up;
        }
    }
}

/// `carryPlaylists`: the playlists the source project's surfaces name, those the destination lacks and the source has.
fn playlists(
    src_root: &str,
    dest_root: &str,
    src_project: &str,
    written: &mut Written,
    out: &mut Vec<Entry>,
) -> Result<Vec<String>, WsError> {
    let mut carried = Vec::new();
    let base = join(&[dest_root, PLAYLISTS_DIR]);
    let from_base = join(&[src_root, PLAYLISTS_DIR]);
    for s in project_surfaces(src_project)? {
        let Some(rel) = s.playlist.as_deref().and_then(playlist_path_of) else {
            continue;
        };
        let (to, from) = (join(&[dest_root, &rel]), join(&[src_root, &rel]));
        // A name climbing out of `_playlists` by a backslash is a path the TypeScript writes outside; it is skipped.
        if !is_within(&base, &to) || !is_within(&from_base, &from) {
            continue;
        }
        if written.exists(&to) || fs::metadata(&from).is_err() {
            continue;
        }
        let e = Entry::File { from, to };
        written.note(&e);
        out.push(e);
        carried.push(rel);
    }
    Ok(carried)
}

/// `carryModules`: the `@workspace` modules the copy names, and those they name in turn, into the destination's `_modules`.
#[allow(clippy::too_many_arguments)]
fn modules(
    src_root: &str,
    dest_root: &str,
    project_entries: &[Entry],
    dest_project: &str,
    written: &mut Written,
    out: &mut Vec<Entry>,
    limits: &Limits,
    seen: &mut u64,
) -> Result<Vec<String>, WsError> {
    let mut carried = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    let mut todo = Todo::default();
    refs_of_entries(dest_project, project_entries, &mut todo, limits)?;
    let (from_base, to_base) = (
        join(&[src_root, MODULES_DIR]),
        join(&[dest_root, MODULES_DIR]),
    );
    while let Some(path) = todo.take() {
        *seen += 1;
        if *seen > limits.max_seen {
            return Err(Declined::TooMany.into());
        }
        if !done.insert(path.clone()) {
            continue;
        }
        for rel in [path.clone(), format!("{path}.k7x")] {
            let (from, to) = (join(&[&from_base, &rel]), join(&[&to_base, &rel]));
            // Every module's source and destination stay inside `_modules` (1690).
            if !is_within(&from_base, &from) || !is_within(&to_base, &to) {
                continue;
            }
            if fs::metadata(&from).is_err() || written.exists(&to) {
                continue;
            }
            if fs::metadata(&from).is_ok_and(|m| m.is_dir()) {
                let entries = plan_tree(&from, &to, &|_| true, limits, seen)?;
                refs_of_entries(&to, &entries, &mut todo, limits)?;
                for e in entries {
                    written.note(&e);
                    out.push(e);
                }
            } else {
                refs_in(&text_of(&from, limits)?, &mut todo);
                let e = Entry::File { from, to };
                written.note(&e);
                out.push(e);
            }
            carried.push(format!("{MODULES_DIR}/{rel}"));
            break;
        }
    }
    Ok(carried)
}

/// The copy judged, in the TypeScript's order, with nothing written: both ends' names and presence, the source's
/// guards, the destination's, the name, then every entry, playlist and module it would write.
pub fn plan_copy(home: &Home, ask: &CopyAsk<'_>, limits: &Limits) -> Result<CopyPlan, WsError> {
    let (_, src_root) = end(&home, ask.from)?;
    let (dest_name, dest_root) = end(&home, ask.to)?;
    let src_space = Space::open(&home, &src_root);
    let src = src_space.safe_path(ask.path, &Disk)?;
    if is_recycled(&src_space, &src) {
        return Err(WsError::Refused(format!(
            "project is in the recycle bin: {}",
            ask.path
        )));
    }
    if !is_project_dir(&src) {
        return Err(WsError::Refused(format!(
            "not a project (the folder must end in .k7): {}",
            ask.path
        )));
    }
    let dest_space = Space::open(&home, &dest_root);
    if is_project_dir(&dest_root) {
        return Err(WsError::Refused(format!(
            "{dest_name} is itself a project — copy into a workspace that holds several"
        )));
    }
    let name = copy_name(&dest_name, &basename(&src), ask.name)?;
    let dest = dest_space.safe_path(&name, &Disk)?;
    if dest == dest_space.root() {
        return Err(WsError::Refused("project name required".into()));
    }
    if fs::metadata(&dest).is_ok() {
        return Err(WsError::Refused(format!("\"{name}\" already exists")));
    }
    let mut seen = 0;
    let mut entries = plan_project(&src, &dest, limits, &mut seen)?;
    let mut written = Written::new(&dest_root);
    for e in &entries {
        written.note(e);
    }
    let mut extra = Vec::new();
    let playlists = playlists(&src_root, &dest_root, &src, &mut written, &mut extra)?;
    let project_entries = entries.clone();
    let modules = modules(
        &src_root,
        &dest_root,
        &project_entries,
        &dest,
        &mut written,
        &mut extra,
        limits,
        &mut seen,
    )?;
    entries.extend(extra);
    Ok(CopyPlan {
        src: src_root,
        dest: dest_root,
        entries,
        project: to_rel(&dest_space, &dest),
        playlists,
        modules,
    })
}

/// The entries written, then the playlists, then the modules; a failure leaves what was written (1654).
pub fn apply_copy(plan: &CopyPlan) -> Result<(), WsError> {
    apply_entries(&plan.entries)
}

/// `{project, playlists?, modules?}`, the answer's literal.
pub fn answer_of(plan: &CopyPlan) -> String {
    let mut v = json!({ "project": plan.project });
    if !plan.playlists.is_empty() {
        v["playlists"] = json!(plan.playlists);
    }
    if !plan.modules.is_empty() {
        v["modules"] = json!(plan.modules);
    }
    v.to_string()
}
