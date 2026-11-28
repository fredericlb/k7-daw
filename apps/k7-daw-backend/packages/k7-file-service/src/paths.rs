//! paths.rs — node's lexical path model, function for function: `js/node/path.ts` and its `isWithin`/`isPlainSegment` rules.
//! Strings only, no filesystem and no `cfg!`: `\` reads as `/` on every host, a `C:/` root exists on every host.
//! What a reader gets wrong: this is never `std::path`; `C:/x` on a Mac is an escape, as node answers it (1020).

use std::borrow::Cow;

/// `p` with `\` read as `/`.
pub fn slashed(p: &str) -> Cow<'_, str> {
    if p.contains('\\') {
        Cow::Owned(p.replace('\\', "/"))
    } else {
        Cow::Borrowed(p)
    }
}

/// The root a slashed path starts with: `C:/`, `/`, or nothing.
fn root_of(p: &str) -> &str {
    let b = p.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/' {
        return &p[..3];
    }
    if p.starts_with('/') { "/" } else { "" }
}

/// Node's `normalizeString`: `.` dropped, `..` folded, kept above the root only when `above` and the path is relative.
fn normalize_string(path: &str, above: bool) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| *l != "..") {
                    out.pop();
                } else if above {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

/// The interpreter's fast path: a non-empty path with no `.` or `..` segment, no `//` and no `\`.
fn clean(p: &str) -> bool {
    !p.is_empty()
        && !p.contains('\\')
        && !p.contains("//")
        && !p.split('/').any(|s| s == "." || s == "..")
}

/// `path.normalize`.
pub fn normalize(p: &str) -> String {
    if clean(p) {
        return p.to_string();
    }
    let s = slashed(p);
    if s.is_empty() {
        return ".".into();
    }
    let root = root_of(&s);
    let trailing = s.ends_with('/');
    let mut body = normalize_string(&s[root.len()..], root.is_empty());
    if body.is_empty() && root.is_empty() {
        body.push('.');
    }
    if !body.is_empty() && trailing {
        body.push('/');
    }
    format!("{root}{body}")
}

/// `path.join`.
pub fn join(parts: &[&str]) -> String {
    let mut out = String::new();
    for p in parts {
        if p.is_empty() {
            continue;
        }
        if !clean(p) || (!out.is_empty() && p.starts_with('/')) {
            return slow_join(parts);
        }
        if out.is_empty() {
            out.push_str(p);
        } else {
            if !out.ends_with('/') {
                out.push('/');
            }
            out.push_str(p);
        }
    }
    if out.is_empty() { ".".into() } else { out }
}

fn slow_join(parts: &[&str]) -> String {
    let kept: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if kept.is_empty() {
        ".".into()
    } else {
        normalize(&kept.join("/"))
    }
}

/// `path.resolve(...parts)` where a path with no root takes `cwd` first.
pub fn resolve_in(cwd: &str, parts: &[&str]) -> String {
    if let Some(last) = parts.last().filter(|l| clean(l) && !root_of(l).is_empty()) {
        let root = root_of(last).len();
        return if last.len() > root && last.ends_with('/') {
            last[..last.len() - 1].to_string()
        } else {
            (*last).to_string()
        };
    }
    let mut acc = String::new();
    let mut i = parts.len() as isize - 1;
    while i >= -1 && root_of(&acc).is_empty() {
        let p = if i >= 0 {
            slashed(parts[i as usize])
        } else {
            slashed(cwd)
        };
        i -= 1;
        if p.is_empty() {
            continue;
        }
        acc = if acc.is_empty() {
            p.into_owned()
        } else {
            format!("{p}/{acc}")
        };
    }
    let root = root_of(&acc);
    let out = format!("{root}{}", normalize_string(&acc[root.len()..], false));
    if out.is_empty() && root.is_empty() {
        ".".into()
    } else {
        out
    }
}

/// `path.resolve(base, rel)`, a relative-only chain taking `/` for node's working folder.
pub fn resolve(base: &str, rel: &str) -> String {
    resolve_in("/", &[base, rel])
}

/// `path.resolve(...parts)`.
pub fn resolve_all(parts: &[&str]) -> String {
    resolve_in("/", parts)
}

/// `path.dirname`.
pub fn dirname(p: &str) -> String {
    let s = slashed(p);
    if s.is_empty() {
        return ".".into();
    }
    let root = root_of(&s);
    let mut end = s.len();
    while end > root.len() && s.as_bytes()[end - 1] == b'/' {
        end -= 1;
    }
    let Some(cut) = s[..end].rfind('/') else {
        return if root.is_empty() {
            ".".into()
        } else {
            root.to_string()
        };
    };
    if cut < root.len() {
        return if root.is_empty() {
            ".".into()
        } else {
            root.to_string()
        };
    }
    let out = &s[..cut.max(root.len())];
    if out.is_empty() {
        root.to_string()
    } else {
        out.to_string()
    }
}

/// `path.basename`.
pub fn basename(p: &str) -> String {
    let s = slashed(p);
    let mut end = s.len();
    while end > 0 && s.as_bytes()[end - 1] == b'/' {
        end -= 1;
    }
    let start = s[..end].rfind('/').map_or(0, |i| i + 1);
    s[start..end].to_string()
}

/// `path.extname`: the last dot not at the start of the name, with the dot.
pub fn extname(p: &str) -> String {
    let base = basename(p);
    match base.rfind('.') {
        Some(dot) if dot > 0 && base != ".." => base[dot..].to_string(),
        _ => String::new(),
    }
}

/// `path.relative`.
pub fn relative(from: &str, to: &str) -> String {
    let a = resolve_all(&[from]);
    let b = resolve_all(&[to]);
    if a == b {
        return String::new();
    }
    let ra = root_of(&a);
    if ra != root_of(&b) {
        return b;
    }
    let pa: Vec<&str> = a[ra.len()..].split('/').filter(|s| !s.is_empty()).collect();
    let pb: Vec<&str> = b[ra.len()..].split('/').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < pa.len() && i < pb.len() && pa[i] == pb[i] {
        i += 1;
    }
    let mut out: Vec<&str> = vec![".."; pa.len() - i];
    out.extend(&pb[i..]);
    out.join("/")
}

/// A path spelt the Windows way: a drive letter or a UNC share, where both separators count and case does not.
fn is_windows_path(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || p.starts_with("\\\\")
}

/// `isWithin`: `path` is `root` itself or below it, both absolute and resolved; case folds only under a Windows root.
pub fn is_within(root: &str, path: &str) -> bool {
    if root.is_empty() || path.is_empty() {
        return false;
    }
    let win = is_windows_path(root);
    let (r, p) = if win {
        (root.to_lowercase(), path.to_lowercase())
    } else {
        (root.to_string(), path.to_string())
    };
    if p == r {
        return true;
    }
    let seps: &[char] = if win { &['\\', '/'] } else { &['/'] };
    seps.iter().any(|s| {
        if r.ends_with(*s) {
            p.starts_with(&r)
        } else {
            p.starts_with(&r) && p[r.len()..].starts_with(*s)
        }
    })
}

/// `isBelow`: `is_within`, the root itself refused.
pub fn is_below(root: &str, path: &str) -> bool {
    is_within(root, path) && !is_within(path, root)
}

/// `path.isAbsolute` of the shim: a `/` or a `C:/` root once `\` reads as `/`.
pub fn is_absolute(p: &str) -> bool {
    !root_of(&slashed(p)).is_empty()
}

/// `climbsOut`: a `relative(root, x)` result that leaves `root`.
pub fn climbs_out(rel: &str) -> bool {
    if rel == ".." || rel.starts_with("../") || rel.starts_with("..\\") {
        return true;
    }
    rel.starts_with('/') || is_windows_path(rel)
}

#[cfg(test)]
mod within_tests {
    use super::{is_within, resolve_all};

    // A root is resolved before it is compared: `resolve_all` slashes it as it does the path.
    #[test]
    fn a_backslash_root_resolved_holds_the_slashed_paths_below_it() {
        let root = resolve_all(&["C:\\ws\\a"]);
        assert!(is_within(&root, "C:/ws/a/b.wav"));
        assert!(!is_within(&root, "C:/ws/ab/b.wav"));
    }
}
