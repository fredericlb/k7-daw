// source/path.rs — paths as the Source spells them: absolute, `/`-separated, a drive letter kept, `.` and `..` resolved.
// A reader would get wrong that nothing here touches a file system: it is lexical, so a link is the disk Source's to chase.

use super::SourceError;
use serde::{Deserialize, Serialize};

/// The path rule of the machine that spelled a path: a Windows host reads `\` as a separator and a drive as absolute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathRule {
    Posix,
    Windows,
}

impl PathRule {
    /// The machine this crate runs on: the one place, with `source/disk.rs`, that asks `cfg!`.
    pub const HOST: PathRule = if cfg!(windows) {
        PathRule::Windows
    } else {
        PathRule::Posix
    };

    pub const fn is_windows(self) -> bool {
        matches!(self, PathRule::Windows)
    }
}

impl Default for PathRule {
    fn default() -> PathRule {
        PathRule::HOST
    }
}

/// An absolute path in one spelling; a NUL or a relative path is `Io`.
pub fn normalize(path: &str) -> Result<String, SourceError> {
    if path.contains('\0') {
        return Err(SourceError::Io("a path holds a NUL".to_owned()));
    }
    let Some((root, rest)) = split_root(path) else {
        return Err(SourceError::Io(format!("{path}: not an absolute path")));
    };
    let mut parts: Vec<&str> = Vec::new();
    for c in rest.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => parts.push(name),
        }
    }
    Ok(format!("{root}{}", parts.join("/")))
}

fn split_root(path: &str) -> Option<(String, &str)> {
    if let Some(unc) = path.strip_prefix("//").filter(|r| !r.starts_with(['/', '?', '.'])) {
        // A UNC root is `//server/share/`: both names are root, so `..` stops there.
        let mut it = unc.splitn(3, '/');
        let (server, share) = (it.next()?, it.next().filter(|s| !s.is_empty())?);
        return Some((format!("//{server}/{share}/"), it.next().unwrap_or("")));
    }
    if let Some(rest) = path.strip_prefix('/') {
        return Some(("/".to_owned(), rest));
    }
    let b = path.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    if drive && (b.len() == 2 || b[2] == b'/') {
        let letter = char::from(b[0].to_ascii_uppercase());
        return Some((format!("{letter}:/"), path.get(3..).unwrap_or("")));
    }
    None
}

/// The length of a normalised path's root: `/` is 1, `C:/` is 3, `//srv/share/` is 12.
fn root_len(path: &str) -> usize {
    if path.starts_with("//") {
        let ends = path.match_indices('/').nth(3).map(|(i, _)| i + 1);
        return ends.unwrap_or(path.len());
    }
    if path.starts_with('/') { 1 } else { 3.min(path.len()) }
}

/// How many names a normalised path holds below its root.
pub fn depth(path: &str) -> usize {
    let rest = path.get(root_len(path)..).unwrap_or("");
    if rest.is_empty() {
        0
    } else {
        rest.bytes().filter(|b| *b == b'/').count() + 1
    }
}

/// The first `names` names of a normalised path, root included, as a slice: no copy, and no walk past them.
pub fn head(path: &str, names: usize) -> &str {
    let root = root_len(path);
    if names == 0 {
        return path.get(..root).unwrap_or(path);
    }
    let mut seen = 0;
    for (i, b) in path.bytes().enumerate().skip(root) {
        if b == b'/' {
            seen += 1;
            if seen == names {
                return path.get(..i).unwrap_or(path);
            }
        }
    }
    path
}

/// Whether `hit` accepts a head of the path within `max_names` names and `max_len` bytes: a deep path costs its shallow end.
pub fn any_head(path: &str, max_names: usize, max_len: usize, mut hit: impl FnMut(&str) -> bool) -> bool {
    let root = root_len(path);
    if root <= max_len && path.get(..root).is_some_and(&mut hit) {
        return true;
    }
    let mut names = 0;
    for (i, b) in path.bytes().enumerate().skip(root) {
        if b == b'/' {
            names += 1;
            if names > max_names || i > max_len {
                return false;
            }
            if path.get(..i).is_some_and(&mut hit) {
                return true;
            }
        }
    }
    path.len() > root && names < max_names && path.len() <= max_len && hit(path)
}

/// `name` under `dir`, both already normalised.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// `rel` under `root`, lexically: None when a `..` of `rel` climbs out of `root`, `root` itself when `rel` names it.
pub fn join_within(root: &str, rel: &str) -> Option<String> {
    // An absolute `rel` names a place of its own, never one below `root`.
    if rel.contains('\0') || rel.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for c in rel.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(if parts.is_empty() {
        root.to_owned()
    } else {
        join(root, &parts.join("/"))
    })
}

/// The folder holding `path`, or None for a root.
pub fn parent(path: &str) -> Option<String> {
    if path.starts_with("//") {
        return parent_str(path).map(str::to_owned);
    }
    let cut = path.rfind('/')?;
    let head = path.get(..cut)?;
    if head.is_empty() || head.ends_with(':') {
        return (path.len() > cut + 1).then(|| format!("{head}/"));
    }
    Some(head.to_owned())
}

/// The folder holding `path` as a slice, or None for a root: `parent` without the copy.
pub fn parent_str(path: &str) -> Option<&str> {
    if path.starts_with("//") {
        let root = root_len(path);
        let below = path.get(root..)?.trim_end_matches('/');
        let cut = below.rfind('/').map_or(root, |i| root + i);
        return (!below.is_empty()).then(|| path.get(..cut)).flatten();
    }
    let cut = path.rfind('/')?;
    let head = path.get(..cut)?;
    if head.is_empty() || head.ends_with(':') {
        return (path.len() > cut + 1).then(|| path.get(..=cut)).flatten();
    }
    Some(head)
}

/// Whether `path` is `root` or lies below it; the two are normalised.
pub fn is_within(root: &str, path: &str) -> bool {
    path == root || (path.starts_with(root) && (root.ends_with('/') || path.as_bytes().get(root.len()) == Some(&b'/')))
}

/// Whether a normalised path holds what Windows reads as more than a name: a `\` separator or a `:` past the drive.
/// The drive is `C:/x` or, in the posix form a host edge computes on, `/C:/x` and `/UNC:/srv/share/x`.
pub fn windows_hazard(path: &str) -> bool {
    let b = path.as_bytes();
    let rest = if b.get(1) == Some(&b':') {
        path.get(2..).unwrap_or("")
    } else if let Some(unc) = path.strip_prefix("/UNC:").filter(|r| r.starts_with('/')) {
        unc
    } else if b.first() == Some(&b'/')
        && b.get(1).is_some_and(u8::is_ascii_alphabetic)
        && b.get(2) == Some(&b':')
        && b.get(3) == Some(&b'/')
    {
        path.get(3..).unwrap_or("")
    } else {
        path
    };
    rest.contains('\\') || rest.contains(':')
}

/// The mounts a Source serves, normalised once.
#[derive(Clone, Debug, Default)]
pub struct Mounts {
    roots: Vec<String>,
    /// Mounts that name ONE file: each admits that path alone.
    files: Vec<String>,
}

impl Mounts {
    pub fn new<S: AsRef<str>>(mounts: &[S]) -> Result<Mounts, SourceError> {
        let roots = mounts
            .iter()
            .map(|m| normalize(m.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Mounts {
            roots,
            files: Vec::new(),
        })
    }

    /// Folder mounts, and file mounts that each admit the one path they name.
    pub fn with_files<S: AsRef<str>, F: AsRef<str>>(dirs: &[S], files: &[F]) -> Result<Mounts, SourceError> {
        let mut m = Mounts::new(dirs)?;
        m.files = files
            .iter()
            .map(|f| normalize(f.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(m)
    }

    /// The file mounts.
    pub fn file_roots(&self) -> &[String] {
        &self.files
    }

    /// Whether `path` is exactly a file mount and lies in no folder mount.
    pub fn is_file_mount(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path) && !self.roots.iter().any(|r| is_within(r, path))
    }

    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// Every mount `path` lies in, lexically.
    pub fn containing<'a>(&'a self, path: &str) -> Vec<&'a str> {
        self.roots
            .iter()
            .chain(self.files.iter().filter(|f| f.as_str() == path))
            .map(String::as_str)
            .filter(|r| is_within(r, path))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unc_root_keeps_both_slashes_and_stops_dotdot_at_the_share() {
        assert_eq!(normalize("//srv/share").unwrap(), "//srv/share/");
        assert_eq!(normalize("//srv/share/a/./b/../c").unwrap(), "//srv/share/a/c");
        assert_eq!(normalize("//srv/share/../../x").unwrap(), "//srv/share/x");
        assert_eq!(normalize("///x").unwrap(), "/x");
        assert!(normalize("//srv").is_err());
        // A verbatim or device prefix is no server: slashed, it stays the hazard it was.
        assert_eq!(normalize("//?/C:/x").unwrap(), "/?/C:/x");
        assert!(!windows_hazard("//srv/share/a"));
    }

    #[test]
    fn the_posix_form_of_a_drive_or_a_share_is_no_hazard_but_a_colon_past_it_is() {
        assert!(!windows_hazard("/C:/p/a/b") && !windows_hazard("/UNC:/srv/share/a"));
        assert!(windows_hazard("/C:/p/a:stream") && windows_hazard("/C:/p/a\\b"));
        assert!(windows_hazard("/UNC:/srv/share/a:s"));
        // A lane name `c:x` or `UNC:x` is a name with a colon, not a drive.
        assert!(windows_hazard("/c:x") && windows_hazard("/UNC:x"));
    }

    #[test]
    fn a_unc_path_has_a_root_a_depth_a_head_and_parents() {
        let p = "//srv/share/a/b";
        assert_eq!((depth("//srv/share/"), depth(p)), (0, 2));
        assert_eq!(head(p, 0), "//srv/share/");
        assert_eq!(head(p, 1), "//srv/share/a");
        assert_eq!(parent(p).as_deref(), Some("//srv/share/a"));
        assert_eq!(parent("//srv/share/a").as_deref(), Some("//srv/share/"));
        assert_eq!(parent("//srv/share/"), None);
        assert_eq!(parent_str("//srv/share/a"), Some("//srv/share/"));
        assert!(is_within("//srv/share/", p) && !is_within("//srv/share/", "//srv/other/a"));
        assert_eq!(join_within("//srv/share/", "a/../b").as_deref(), Some("//srv/share/b"));
    }
}
