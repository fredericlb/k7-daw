// library_forks.rs — the FORKS of a library: the `_modules` and `@account` modules extending one of its modules, and which would inherit something new on an upgrade.
// A reader would get wrong what "moved" means: the module they extend differs in content between two versions, by a digest of its folder.

use crate::confine::Env;
use crate::error::Res;
use crate::fs::Fs;
use crate::paths::{join, relative};
use crate::shared_folders::{ACCOUNT_LIBRARY, SHARED_MODULES_DIR, account_library_home, library_version_root};
use crate::workspace_doc::ws_data;
use sha2::{Digest, Sha256};
use song::Kind;

/// One fork: the workspace module, and the library module it extends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFork {
    /// Where it is: `_modules/<path>` or `@account/<path>`.
    pub module: String,
    /// What it extends, `@<library>/<path>`.
    pub base: String,
}

/// `@<library>/<rest>` split into its library and the rest, or None for a ref no library answers to.
#[must_use]
pub fn library_ref_parts(r: &str) -> Option<(&str, &str)> {
    let body = r.strip_prefix('@')?;
    let slash = body.find('/')?;
    let (library, rest) = (&body[..slash], &body[slash + 1..]);
    let broken = rest.is_empty() || rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']);
    let named = !library.is_empty() && (crate::confine::is_library_name(library) || library == ACCOUNT_LIBRARY);
    (named && !broken).then_some((library, rest))
}

/// `/^module.k7x$/`: the dot is any character but a line terminator.
fn is_module_file(name: &str) -> bool {
    name.strip_prefix("module").is_some_and(|r| {
        let mut c = r.chars();
        c.next()
            .is_some_and(|d| !matches!(d, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
            && c.as_str() == "k7x"
    })
}

fn module_files(fs: &dyn Fs, dir: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs.list(dir) else { return };
    for e in entries {
        if e.name.starts_with('.') {
            continue;
        }
        let abs = join(&[dir, &e.name]);
        if is_module_file(&e.name) {
            out.push(abs);
        } else if e.kind == Kind::Dir {
            module_files(fs, &abs, out);
        }
    }
}

/// The workspace's modules at `repo_dir`, and the account's, that extend a module of `library`.
#[must_use]
pub fn library_forks(fs: &dyn Fs, env: &Env, repo_dir: &str, library: &str) -> Vec<LibraryFork> {
    let roots = [
        (join(&[repo_dir, SHARED_MODULES_DIR]), SHARED_MODULES_DIR.to_owned()),
        (
            join(&[&account_library_home(env), "modules"]),
            format!("@{ACCOUNT_LIBRARY}"),
        ),
    ];
    let mut forks = vec![];
    for (root, label) in roots {
        let mut files = vec![];
        module_files(fs, &root, &mut files);
        for file in files {
            let Ok(doc) = ws_data(fs, &file) else { continue };
            let Some(k7x::edit::Value::Str(ext)) = doc.get("extends") else {
                continue;
            };
            if library_ref_parts(ext).map(|(l, _)| l) != Some(library) {
                continue;
            }
            let at = relative(&root, &crate::paths::dirname(&file)).replace('\\', "/");
            forks.push(LibraryFork {
                module: format!("{label}/{at}"),
                base: ext.clone(),
            });
        }
    }
    forks.sort_by(|a, b| a.module.cmp(&b.module));
    forks
}

fn walk_digest(fs: &dyn Fs, dir: &str, at: &str, hash: &mut Sha256) {
    let Ok(mut entries) = fs.list(at) else { return };
    entries.sort_by_key(|e| e.name.encode_utf16().collect::<Vec<u16>>());
    for e in entries {
        if e.name.starts_with('.') {
            continue;
        }
        let abs = join(&[at, &e.name]);
        if e.kind == Kind::Dir {
            walk_digest(fs, dir, &abs, hash);
        } else if let Ok(bytes) = fs.read(&abs, song::Span::All) {
            hash.update(relative(dir, &abs).as_bytes());
            hash.update([0u8]);
            hash.update(bytes);
        }
    }
}

/// A digest of a library module's folder at one version, or None when that version has no such module.
#[must_use]
pub fn library_module_digest(fs: &dyn Fs, env: &Env, library: &str, version: &str, r: &str) -> Option<String> {
    let (_, rest) = library_ref_parts(r)?;
    // A ref may name one node of the module (`…/x:warm`); the folder is the module's.
    let path = rest.split(':').next().unwrap_or(rest);
    let dir = join(&[&library_version_root(fs, env, library, version), "modules", path]);
    if !fs.exists(&dir) {
        return None;
    }
    let mut hash = Sha256::new();
    walk_digest(fs, &dir, &dir, &mut hash);
    Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The forks whose base differs between `from` and `to`; gone from `to` counts as differing.
///
/// # Errors
/// Never in practice: kept as `Res` for the host's uniform call.
pub fn forks_whose_base_moved(
    fs: &dyn Fs,
    env: &Env,
    repo_dir: &str,
    library: &str,
    from: &str,
    to: &str,
) -> Res<Vec<LibraryFork>> {
    Ok(library_forks(fs, env, repo_dir, library)
        .into_iter()
        .filter(|f| {
            library_module_digest(fs, env, library, from, &f.base)
                != library_module_digest(fs, env, library, to, &f.base)
        })
        .collect())
}
