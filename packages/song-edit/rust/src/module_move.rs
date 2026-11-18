// module_move.rs — moving a library module to another folder of the same library and type, repointing every ref that named it.
// A reader would get wrong that only the roots handed in are repointed: another account's workspace keeps the old ref.

use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::module_io::module_file_for;
use crate::paths::{basename, climbs_out, dirname, join, relative};
use crate::sugar::is_doc_file;
use crate::workspace_doc::{ws_text, ws_write_text};
use k7x::edit::{Document, PathSeg, Tree, Value, WriteOptions};
use song::modules::types::ENGINES_DIR;
use song::modules::types::type_meta;
use song::{Kind, SourceError};

/// Where a move lands, and the two refs it trades.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleMove {
    pub from: String,
    pub to: String,
    pub dest: String,
}

/// A library by name and root folder.
#[derive(Clone, Copy, Debug)]
pub struct Library<'a> {
    pub name: &'a str,
    pub root: &'a str,
}

/// Folders a walk for refs never enters.
const SKIP: [&str; 5] = [".git", "node_modules", "_history", "_recycle_bin", "_libraries"];

fn ref_of(library: &str, segs: &[&str]) -> String {
    format!("@{library}/{}", segs.join("/"))
}

/// The segments of `abs` below `modules`: `Some([])` for the folder itself, None outside it.
fn inside(rel: &str) -> Option<Vec<&str>> {
    if rel.is_empty() {
        Some(vec![])
    } else if climbs_out(rel) {
        None
    } else {
        Some(rel.split('/').collect())
    }
}

/// Plan the move of the module folder `module_abs` into `dest_abs`, both inside `library`'s `modules/`.
///
/// # Errors
/// The reason the move cannot be one.
pub fn plan_module_move(fs: &dyn Fs, library: Library, module_abs: &str, dest_abs: &str) -> Res<ModuleMove> {
    let modules = join(&[library.root, "modules"]);
    let (rel_from, rel_to) = (relative(&modules, module_abs), relative(&modules, dest_abs));
    let from = inside(&rel_from);
    let to = inside(&rel_to);
    let Some(from) = from.filter(|f| f.len() >= 2 && module_file_for(fs, module_abs, "", "module").is_some()) else {
        return fail("only a module of a library's modules/ can be moved");
    };
    if from.contains(&ENGINES_DIR) {
        return fail("an engine moves with its module, not on its own");
    }
    let Some(to) = to.filter(|t| !t.is_empty()) else {
        return fail(format!("drop it on a folder of {}'s modules/", library.name));
    };
    if to[0] != from[0] {
        return fail(format!(
            "a module stays in its type: this one is {}/, not {}/",
            from[0], to[0]
        ));
    }
    if type_meta(to[0]).is_none() {
        return fail(format!("{}/ is not a module type", to[0]));
    }
    if to.len() > 2 {
        return fail("a module sits in its type folder or one category below it");
    }
    if to.contains(&ENGINES_DIR) || module_file_for(fs, dest_abs, "", "module").is_some() {
        return fail("drop it on a folder, not inside another module");
    }
    if dirname(module_abs) == dest_abs {
        return fail("it is already there");
    }
    let name = basename(module_abs);
    let dest = join(&[dest_abs, name]);
    if fs.exists(&dest) {
        return fail(format!("there is already a {name} in {}/", to.join("/")));
    }
    let mut target = to.clone();
    target.push(name);
    Ok(ModuleMove {
        from: ref_of(library.name, &from),
        to: ref_of(library.name, &target),
        dest,
    })
}

/// The ref a path inside a library's `modules/` stands for: a module's, or the prefix of every one below a folder.
#[must_use]
pub fn ref_at(library: Library, abs: &str) -> Option<String> {
    let rel = relative(&join(&[library.root, "modules"]), abs);
    if rel.is_empty() || climbs_out(&rel) {
        return None;
    }
    Some(ref_of(library.name, &rel.split('/').collect::<Vec<_>>()))
}

/// Refused, with the reason: an engine is picked by its folder's NAME, which no ref carries.
///
/// # Errors
/// A path through an `engines/` folder.
pub fn assert_not_engine(library_root: &str, abs: &str) -> Res<()> {
    let rel = relative(&join(&[library_root, "modules"]), abs);
    if rel.split('/').any(|s| s == ENGINES_DIR) {
        return fail(
            "an engine is picked by its name (`engine: <name>` in every node using it), which renaming would strand — make a new engine instead",
        );
    }
    Ok(())
}

/// `s` repointed when it IS the ref, or the ref followed by a node (`:`), a port (`#`) or, `under`, more path.
fn repointed(s: &str, from: &str, to: &str, under: bool) -> Option<String> {
    let rest = s.strip_prefix(from)?;
    let follows = rest.is_empty() || rest.starts_with([':', '#']);
    (follows || (under && rest.starts_with('/'))).then(|| format!("{to}{rest}"))
}

fn walk(
    t: &Tree,
    path: &mut Vec<PathSeg>,
    from: &str,
    to: &str,
    under: bool,
    vals: &mut Vec<(Vec<PathSeg>, String)>,
    keys: &mut Vec<(Vec<PathSeg>, String)>,
) {
    match t {
        Tree::Scalar {
            value: Value::Str(s), ..
        } => {
            if let Some(v) = repointed(s, from, to, under) {
                vals.push((path.clone(), v));
            }
        }
        Tree::Scalar { .. } => {}
        Tree::List { items, .. } => {
            for (i, item) in items.iter().enumerate() {
                path.push(PathSeg::Index(i));
                walk(item, path, from, to, under, vals, keys);
                path.pop();
            }
        }
        Tree::Map(m) => {
            for e in &m.entries {
                path.push(PathSeg::Key(e.key.clone()));
                if let Some(k) = repointed(&e.key, from, to, under) {
                    keys.push((path.clone(), k));
                }
                walk(&e.value, path, from, to, under, vals, keys);
                path.pop();
            }
        }
    }
}

/// One document's refs repointed, every string value and map key that names `from`; whether anything changed.
///
/// # Errors
/// An edit the document refuses.
pub fn repoint_doc_refs(doc: &mut Document, from: &str, to: &str, under: bool) -> Res<bool> {
    if !doc.errors().is_empty() {
        return Ok(false);
    }
    let (mut vals, mut keys) = (vec![], vec![]);
    doc.node_at(&[], |n| {
        if let Some(root) = n {
            walk(root, &mut vec![], from, to, under, &mut vals, &mut keys);
        }
    })?;
    for (path, v) in &vals {
        doc.set_in(path, &Value::Str(v.clone()), WriteOptions::default())?;
    }
    // Deepest first, so a renamed key never moves a path still waiting to be renamed.
    keys.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    for (path, k) in &keys {
        // The map already holds the new key: renaming would merge two entries, so this one is left.
        let _ = doc.rename_key(path, k);
    }
    Ok(!vals.is_empty() || !keys.is_empty())
}

fn walk_dir(fs: &mut dyn Fs, dir: &str, refs: (&str, &str, bool), root: bool, changed: &mut Vec<String>) -> Res<()> {
    let (from, to, under) = refs;
    let entries = match fs.list(dir) {
        Ok(entries) => entries,
        // A root that cannot be listed fails the repoint, its refs would be stranded in silence; a sub-folder is skipped.
        Err(e) if root && !matches!(e, SourceError::NotFound | SourceError::Missing) => {
            return Err(io_error(&e, "scandir", dir));
        }
        Err(_) => return Ok(()),
    };
    for e in entries {
        if SKIP.contains(&e.name.as_str()) {
            continue;
        }
        let abs = join(&[dir, &e.name]);
        let Ok(st) = fs.stat(&abs) else { continue };
        if st.kind == Kind::Dir {
            walk_dir(fs, &abs, refs, false, changed)?;
        } else if is_doc_file(&e.name) {
            let mut doc = Document::new(&ws_text(fs, &abs)?);
            // A file that does not read is left as it is: its author has a syntax error to fix first.
            if repoint_doc_refs(&mut doc, from, to, under)? {
                ws_write_text(fs, &abs, doc.text())?;
                changed.push(abs);
            }
        }
    }
    Ok(())
}

/// Rewrite `from` to `to` in every workspace file under `roots`; the files changed, absolute. `under` also reaches every ref below `from`.
///
/// # Errors
/// A file that cannot be read or written, or a root that cannot be listed (one that is absent is skipped).
pub fn repoint_refs(fs: &mut dyn Fs, roots: &[String], from: &str, to: &str, under: bool) -> Res<Vec<String>> {
    let mut changed = vec![];
    let mut seen: Vec<&String> = vec![];
    for r in roots {
        if !seen.contains(&r) {
            seen.push(r);
            walk_dir(fs, r, (from, to, under), true, &mut changed)?;
        }
    }
    Ok(changed)
}
