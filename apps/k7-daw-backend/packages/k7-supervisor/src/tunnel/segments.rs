//! segments.rs — what a server's path may become on this machine: a name that can only name a child, joined
//! under a root, and the way back from a local path. A server is not trusted: nothing here lets a path
//! climb out of its root, name a Windows device, or alias another name once a Windows copy is made.

use std::path::{Component, Path, PathBuf};

/// Whether Windows would open this name as a device (CON, NUL, COM1, LPT1 …), whatever its case or extension.
pub fn reserved_device_name(seg: &str) -> bool {
    let first = seg.split('.').next().unwrap_or("");
    let base = first.trim_end_matches(' ').to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&base.as_str()) {
        return true;
    }
    let Some(n) = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"))
    else {
        return false;
    };
    matches!(n.chars().collect::<Vec<_>>().as_slice(), [c] if c.is_ascii_digit() || "\u{b9}\u{b2}\u{b3}".contains(*c))
}

/// One name between separators that can only name a child: no `.`, `..`, separator, drive, control char or device name.
pub fn safe_segment(seg: &str) -> bool {
    if reserved_device_name(seg) {
        return false;
    }
    // Dots and spaces alone are refused whole: Windows trims `.. ` to `..`, and `...` names nothing anywhere.
    if seg.chars().all(|c| c == '.' || c == ' ') {
        return false;
    }
    if seg
        .bytes()
        .any(|c| c < 0x20 || c == 0x7f || c == b'/' || c == b'\\' || c == b':')
    {
        return false;
    }
    !(cfg!(windows) && (seg.ends_with('.') || seg.ends_with(' ')))
}

/// `p` reduced lexically: no `.`, and a `..` cancelling the name before it.
fn normalised(p: &Path) -> Vec<Component<'_>> {
    let mut out: Vec<Component> = Vec::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out
}

/// Whether `p` is `root` or below it, compared element by element, never as a string prefix.
fn lexically_inside(root: &Path, p: &Path) -> bool {
    let (a, b) = (normalised(root), normalised(p));
    b.len() >= a.len()
        && a.iter().zip(&b).all(|(x, y)| x == y)
        && !b[a.len()..].contains(&Component::ParentDir)
}

/// `rest` — the `/`-separated part of a server path below its root — under `local_root`, or None if it could leave it.
pub fn join_under(local_root: &str, rest: &str) -> Option<String> {
    let root = PathBuf::from(local_root);
    let mut out = root.clone();
    for seg in rest.split('/') {
        if !safe_segment(seg) {
            return None;
        }
        out.push(seg);
    }
    lexically_inside(&root, &out).then(|| out.to_string_lossy().into_owned())
}

/// The `/`-separated part of `local` below `local_root` ("" for the root itself), or None if it is not lexically inside.
pub fn relative_under(local_root: &str, local: &str) -> Option<String> {
    if local == local_root {
        return Some(String::new());
    }
    let prefix = format!("{local_root}{}", std::path::MAIN_SEPARATOR);
    let below = local.strip_prefix(&prefix)?;
    let seps: &[char] = if cfg!(windows) { &['\\', '/'] } else { &['/'] };
    let mut rel: Vec<&str> = Vec::new();
    for seg in below.split(seps) {
        if !safe_segment(seg) {
            return None;
        }
        rel.push(seg);
    }
    lexically_inside(Path::new(local_root), Path::new(local)).then(|| rel.join("/"))
}
