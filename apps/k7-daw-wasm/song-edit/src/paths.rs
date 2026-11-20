// paths.rs — node:path's posix half over absolute `/` paths: join and resolve fold `..`, extname keeps a dotfile whole.
// A reader would get wrong that a path here is never relative to a working directory: the host always gives absolute ones.

use crate::winpath::{UNC, is_drive, is_win_form};

/// `path` with `.`, `..` and repeated slashes folded; a leading `/` is kept and a relative path stays relative.
/// A drive (`/C:`) or a share (`/UNC:/srv/share`) is a floor `..` cannot climb past.
pub fn resolve(path: &str) -> String {
    let abs = path.starts_with('/');
    let mut parts: Vec<String> = Vec::new();
    let mut floor = 0;
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if parts.len() > floor && parts.last().is_some_and(|l| l != "..") {
                    parts.pop();
                } else if !abs {
                    parts.push("..".to_owned());
                }
            }
            name => {
                if abs && parts.is_empty() && is_drive(name) {
                    parts.push(name.to_ascii_uppercase());
                    floor = 1;
                } else {
                    parts.push(name.to_owned());
                    if abs && parts[0] == UNC {
                        floor = parts.len().min(3);
                    }
                }
            }
        }
    }
    let body = parts.join("/");
    if abs {
        format!("/{body}")
    } else if body.is_empty() {
        ".".to_owned()
    } else {
        body
    }
}

/// `path.join`: the parts glued with `/` and folded; empty parts are skipped.
pub fn join(parts: &[&str]) -> String {
    let glued: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if glued.is_empty() {
        ".".to_owned()
    } else {
        resolve(&glued.join("/"))
    }
}

/// The last name of a path, a trailing slash ignored.
pub fn basename(path: &str) -> &str {
    let t = path.trim_end_matches('/');
    t.rsplit('/').next().unwrap_or("")
}

/// `path.dirname`: `/` for a top-level name, `.` for a bare one.
pub fn dirname(path: &str) -> String {
    let t = path.trim_end_matches('/');
    match t.rfind('/') {
        None => ".".to_owned(),
        Some(0) => "/".to_owned(),
        Some(i) => t[..i].to_owned(),
    }
}

/// `path.extname`: from the last dot of the name, empty when the name starts with that dot or has none.
pub fn extname(path: &str) -> &str {
    let name = basename(path);
    match name.rfind('.') {
        Some(i) if i > 0 => &name[i..],
        _ => "",
    }
}

/// `basename(path, ext)`: the name without `ext` when it ends with it and is longer.
pub fn basename_without<'a>(path: &'a str, ext: &str) -> &'a str {
    let name = basename(path);
    if !ext.is_empty() && name.len() > ext.len() && name.ends_with(ext) {
        &name[..name.len() - ext.len()]
    } else {
        name
    }
}

/// `path.relative(from, to)` over two absolute, resolved paths.
pub fn relative(from: &str, to: &str) -> String {
    let a: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let fold = is_win_form(from) && is_win_form(to);
    let same = |x: &&str, y: &&str| {
        if fold {
            x.to_lowercase() == y.to_lowercase()
        } else {
            x == y
        }
    };
    let common = a.iter().zip(&b).take_while(|(x, y)| same(x, y)).count();
    let mut out: Vec<&str> = vec![".."; a.len() - common];
    out.extend(&b[common..]);
    out.join("/")
}

/// Whether a `relative` result leaves its root: `..` first, or an absolute path.
pub fn climbs_out(rel: &str) -> bool {
    rel == ".."
        || rel.starts_with("../")
        || rel.starts_with("..\\")
        || rel.starts_with('/')
        || rel.starts_with("\\\\")
        || {
            let b = rel.as_bytes();
            b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
        }
}

/// Whether `path` is `root` or below it, both absolute and resolved; on a drive or a share the case is ignored.
pub fn is_within(root: &str, path: &str) -> bool {
    if root.is_empty() || path.is_empty() {
        return false;
    }
    let fold = is_win_form(root) && is_win_form(path);
    let (root, path) = if fold {
        (root.to_lowercase(), path.to_lowercase())
    } else {
        (root.to_owned(), path.to_owned())
    };
    path == root
        || path.starts_with(&if root.ends_with('/') {
            root.clone()
        } else {
            format!("{root}/")
        })
}

/// Whether two resolved absolute paths name the same place; on a drive or a share the case is ignored.
pub fn same_path(a: &str, b: &str) -> bool {
    a == b || (is_win_form(a) && is_win_form(b) && a.to_lowercase() == b.to_lowercase())
}
