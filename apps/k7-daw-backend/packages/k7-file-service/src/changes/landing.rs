//! landing.rs — a NEW project folder under the workspace (P9's import): its samples came first through `/upload`, the set writes
//! the texts. What a reader gets wrong: the folder is removed when the set does not land, unless it already holds a project.

use std::path::Path;

use super::Refusal;
use super::plan::{Act, Planned};
use crate::filter::skip;
use crate::paths::{is_within, join, resolve_all};

/// The most entries `holds_project` looks at: a folder of samples is flat, and a project names its first `.k7x` early.
const MAX_WALK: usize = 20_000;

/// The folder a set asks to create: one plain segment of the workspace, not a shared or hidden name, and only with an empty `at`.
pub fn folder_name(at: &str, name: &str) -> Result<String, Refusal> {
    let bad = |s: String| Refusal::Bad(s);
    if !at.is_empty() {
        return Err(bad(
            "create: a new project folder takes the workspace as `at`".into(),
        ));
    }
    let plain = !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', '\0'])
        && name != ".."
        && !name.starts_with('_')
        && !skip(name);
    if !plain {
        return Err(bad(format!(
            "create: '{name}' is not a name a project folder can take"
        )));
    }
    Ok(name.to_string())
}

/// Whether `root` is the demos' workspace, which takes no new project.
pub fn is_examples(root: &str) -> bool {
    Path::new(root).file_name().and_then(|n| n.to_str()) == Some(crate::workspaces::EXAMPLES)
}

/// Whether `dir` holds a `.k7x` file anywhere below it: a project is there already, and the set must not mix into it.
pub fn holds_project(dir: &str) -> bool {
    let mut stack = vec![Path::new(dir).to_path_buf()];
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in read.flatten() {
            seen += 1;
            if seen > MAX_WALK {
                return true;
            }
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() && !kind.is_symlink() {
                stack.push(e.path());
            } else if e
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".k7x")
            {
                return true;
            }
        }
    }
    false
}

/// The 409 for a folder that is a project already.
pub fn taken(name: &str) -> Refusal {
    Refusal::Taken(format!("'{name}' already holds a project"))
}

/// Every written file lies in the new folder or is a shared text: judged on the RESOLVED paths, `..` already folded by the gates.
pub fn inside(
    root: &str,
    name: &str,
    planned: &Planned,
    shared: &dyn Fn(&str) -> bool,
) -> Result<(), Refusal> {
    let folder = join(&[&resolve_all(&[root]), name]);
    for a in &planned.acts {
        if matches!(a, Act::Read { .. }) {
            continue;
        }
        let mut written = vec![a.target().1.to_string()];
        if let Act::Move { from, .. } = a {
            written.push(from.clone());
        }
        if written
            .iter()
            .any(|abs| !is_within(&folder, abs) && !shared(abs))
        {
            return Err(Refusal::Forbidden(format!(
                "'{}': a set that creates '{name}' writes that folder and the workspace's shared texts only",
                a.target().0
            )));
        }
    }
    Ok(())
}

/// Takes the new folder away, with the samples uploaded into it; a folder that holds a project is somebody's and stays.
pub fn discard(root: &str, name: &str) -> bool {
    let dir = join(&[&resolve_all(&[root]), name]);
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_symlink()) || holds_project(&dir) {
        return false;
    }
    std::fs::remove_dir_all(&dir).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        let d = std::env::temp_dir().join(format!("k7-landing-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.display().to_string()
    }

    // A project that landed under the name while the set failed is kept; samples alone are taken away.
    #[test]
    fn discard_keeps_a_folder_that_holds_a_project() {
        let root = scratch("discard");
        let (mine, theirs) = (join(&[&root, "mine"]), join(&[&root, "theirs"]));
        for d in [&mine, &theirs] {
            std::fs::create_dir_all(join(&[d, "clips"])).unwrap();
            std::fs::write(join(&[d, "clips", "a.wav"]), b"RIFF").unwrap();
        }
        std::fs::write(join(&[&theirs, "project.k7x"]), "project(1)\n").unwrap();
        assert!(discard(&root, "mine") && !Path::new(&mine).exists());
        assert!(!discard(&root, "theirs") && Path::new(&theirs).exists());
        assert!(!discard(&root, "absent"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_demos_workspace_is_the_one_that_takes_no_project() {
        assert!(is_examples("/home/u/workspace/Examples"));
        assert!(!is_examples("/home/u/workspace/songs"));
        assert!(!is_examples("/home/u/workspace/Examples/x"));
    }

    #[test]
    fn the_names_a_folder_can_take() {
        assert_eq!(folder_name("", "hiver.k7").as_deref(), Ok("hiver.k7"));
        for bad in ["", "..", "a/b", "_modules", ".x", "node_modules"] {
            assert!(folder_name("", bad).is_err(), "{bad}");
        }
        assert!(folder_name("p.k7", "x").is_err());
    }
}
