//! paths.rs — which tree paths of a fetched commit may become files of the workspace.
//! What a reader gets wrong: the whole plan is checked before the first file lands, because a path that escapes
//! is written outside the workspace (another account's, on a server) before git's own index refuses it.

use crate::{Error, Result};

const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn part_ok(p: &str) -> bool {
    if p.is_empty() || p == "." || p == ".." || p.ends_with('.') || p.ends_with(' ') {
        return false;
    }
    if p.chars()
        .any(|c| c == '\\' || c == ':' || c == '\0' || c.is_control())
    {
        return false;
    }
    let lower = p.to_ascii_lowercase();
    if lower == ".git" || lower == "git~1" {
        return false;
    }
    let stem = lower.split('.').next().unwrap_or("");
    !RESERVED.contains(&stem)
}

/// Whether a tree path is one k7 may write: relative, inside the workspace, and no name a platform treats as special.
pub fn rel_ok(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && path.split('/').all(part_ok)
}

/// Every path or none: the first that is not ok names the refusal.
pub fn check_all<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<()> {
    match paths.into_iter().find(|p| !rel_ok(p)) {
        Some(p) => Err(Error::BadPath(p.to_string())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_path_inside_the_workspace_with_ordinary_names_is_ok() {
        for ok in [
            "a.k7x",
            "deep/er/take.wav",
            "name with spaces/x.md",
            "a.b/c",
            ".gitignore",
            ".gitattributes",
            "console.txt",
            "conx/y",
        ] {
            assert!(rel_ok(ok), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../x",
            "a/./x",
            "..",
            ".git/config",
            "a/.GIT/x",
            "a/.Git",
            "git~1/x",
            "a\\b",
            "c:/x",
            "a:b",
            "a\0b",
            "nul",
            "NUL.txt",
            "dir/Com1",
            "lpt9.log",
            "x/",
            "trailing./x",
            "space /x",
            "a//b",
        ] {
            assert!(!rel_ok(bad), "{bad:?}");
        }
        assert!(check_all(["a", "b/c"]).is_ok());
        assert_eq!(check_all(["a", "../b"]), Err(Error::BadPath("../b".into())));
    }
}
