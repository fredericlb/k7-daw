//! guards.rs — the refusals every write stands on, function for function and in the order `workspaceEntries.ts` runs them.
//! What a reader gets wrong: a guard's sentence names the caller's path (`to_rel`), never the server's, and the order IS the answer.

use super::EntryError;
use crate::libraries::{ACCOUNT_LIBRARY, Library, library_of, overlay_root};
use crate::paths::{climbs_out, is_within, join, relative, resolve_all};
use crate::rel::to_rel;
use crate::space::Space;

/// The bin a workspace's recycled entries go to, and a library keeps at its own root.
pub const RECYCLE_DIR: &str = "_recycle_bin";
/// The folder of a library's modules, where a ref begins.
const MODULES_DIR: &str = "modules";
/// The folder an engine lives in, picked by its name and so never renamed.
const ENGINES_DIR: &str = "engines";
/// `_modules`, the workspace's shared library, fixed in structure.
const SHARED_MODULES_DIR: &str = "_modules";

fn refused(s: String) -> EntryError {
    EntryError::Refused(s)
}

/// `refuseYamlName`: a workspace document is .k7x, so a name ending `.yml` or `.yaml` is refused.
pub fn refuse_yaml(rel: &str) -> Result<(), EntryError> {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".yml") || lower.ends_with(".yaml") {
        return Err(refused(format!(
            "{rel}: a workspace file is .k7x, not YAML"
        )));
    }
    Ok(())
}

/// `isInSharedModules`: a segment, by either separator, is `_modules`.
pub fn in_shared_modules(path: &str) -> bool {
    path.split(['\\', '/']).any(|s| s == SHARED_MODULES_DIR)
}

/// `isLibraryRootEditable`: the account's own library, or the root the admin's overlay reads (a version never is).
pub fn library_writable(space: &Space<'_>, lib: &Library) -> bool {
    lib.name == ACCOUNT_LIBRARY
        || overlay_root(space.home(), &lib.name)
            .is_some_and(|r| resolve_all(&[&r]) == resolve_all(&[&lib.root]))
}

/// `assertLibraryWritable`: a library is written only by an admin, and only its draft.
pub fn assert_library_writable(space: &Space<'_>, abs: &str, verb: &str) -> Result<(), EntryError> {
    let Some(lib) = library_of(space.home(), abs) else {
        return Ok(());
    };
    if library_writable(space, &lib) {
        return Ok(());
    }
    let rel = to_rel(space, abs);
    let editable = overlay_root(space.home(), &lib.name).is_some();
    Err(refused(if editable {
        format!("can't {verb} '{rel}': a published version never changes — edit the draft")
    } else {
        format!("can't {verb} '{rel}': only the origin's admins edit a library")
    }))
}

/// `guardShared`: the library rule, then `_modules`' fixed structure.
pub fn guard_shared(space: &Space<'_>, abs: &str, verb: &str) -> Result<(), EntryError> {
    assert_library_writable(space, abs, verb)?;
    let rel = to_rel(space, abs);
    if in_shared_modules(&rel) {
        return Err(refused(format!(
            "can't {verb} '{rel}': a shared module is edited in place — change its files directly, or run 'module populate' to rebuild it"
        )));
    }
    Ok(())
}

/// `isRecycled`: in the workspace's own bin, by `is_within`, so a drive root folds case as the shim does (1221).
pub fn is_recycled(space: &Space<'_>, abs: &str) -> bool {
    is_within(&join(&[space.root(), RECYCLE_DIR]), abs)
}

/// `guardMutable`: never the root, a library's root, the bin, or the open project's folder or an ancestor of it.
pub fn guard_mutable(
    space: &Space<'_>,
    active: Option<&str>,
    abs: &str,
    verb: &str,
) -> Result<(), EntryError> {
    if abs == space.root() {
        return Err(refused(format!("can't {verb} the workspace root")));
    }
    // A home spelt with backslashes is the same folder as the resolved `abs`.
    let home = resolve_all(&[&space.home().libraries]);
    let lib = if abs == home {
        None
    } else {
        library_of(space.home(), abs)
    };
    if abs == home || lib.is_some_and(|l| l.root == abs) {
        return Err(refused(format!(
            "can't {verb} '{}': the libraries are listed by the admins, not moved from a workspace",
            to_rel(space, abs)
        )));
    }
    guard_shared(space, abs, verb)?;
    if is_recycled(space, abs) {
        return Err(refused(format!(
            "'{}' is already in the recycle bin",
            to_rel(space, abs)
        )));
    }
    if active.is_some_and(|a| is_within(abs, a)) {
        return Err(refused(format!(
            "can't {verb} the active project's folder — stop it first, then {verb} it"
        )));
    }
    Ok(())
}

/// The segments of `abs` below `<library root>/modules`, when it is one's descendant: `relative` leaving it is `None`.
fn below_modules(lib: &Library, abs: &str) -> Option<Vec<String>> {
    let rel = relative(&join(&[&lib.root, MODULES_DIR]), abs);
    (!rel.is_empty() && !climbs_out(&rel)).then(|| rel.split('/').map(str::to_string).collect())
}

/// `refAt`: the ref a library module answers to, `@<library>/<path below modules/>` — the ref IS the path (1224).
pub fn ref_at(lib: &Library, abs: &str) -> Option<String> {
    below_modules(lib, abs).map(|segs| format!("@{}/{}", lib.name, segs.join("/")))
}

/// `assertNotEngine`: an engine is picked by its folder's name, which no ref carries, so it is never renamed.
pub fn not_engine(lib: &Library, abs: &str) -> Result<(), EntryError> {
    let rel = relative(&join(&[&lib.root, MODULES_DIR]), abs);
    if rel.split('/').any(|s| s == ENGINES_DIR) {
        return Err(refused(
            "an engine is picked by its name (`engine: <name>` in every node using it), which renaming would strand — make a new engine instead"
                .to_string(),
        ));
    }
    Ok(())
}
