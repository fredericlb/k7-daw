// idmove.rs — the one-time move of what the old bundle identifier named to what the new one names, run at startup
// before anything opens or creates a folder. It never merges and never deletes. See docs/app.md.

use std::path::{Path, PathBuf};

/// The identifier the app had before `rocks.k7.daw`.
const OLD_ID: &str = "org.parapluie.studio.engine-tray";
const NEW_ID: &str = "rocks.k7.daw";

/// Moves `old` to `new` when only `old` exists; the returned line is what the log says, None when there is nothing to say.
pub fn move_one(old: &Path, new: &Path) -> Option<String> {
    let exists = |p: &Path| std::fs::symlink_metadata(p).is_ok();
    if !exists(old) {
        return None;
    }
    if exists(new) {
        return Some(format!(
            "identifier move: {} and {} both exist, both kept",
            old.display(),
            new.display()
        ));
    }
    Some(match std::fs::rename(old, new) {
        Ok(()) => format!("identifier move: {} -> {}", old.display(), new.display()),
        Err(e) => format!(
            "identifier move: {} -> {} failed ({e}), the new one is used",
            old.display(),
            new.display()
        ),
    })
}

/// Every (old, new) pair, in order, once per distinct old path (a Mac's data, config and local data are one folder).
pub fn move_all(pairs: &[(PathBuf, PathBuf)]) -> Vec<String> {
    let mut done: Vec<&Path> = Vec::new();
    let mut lines = Vec::new();
    for (old, new) in pairs {
        if done.contains(&old.as_path()) {
            continue;
        }
        done.push(old);
        lines.extend(move_one(old, new));
    }
    lines
}

/// The folders and files this OS keys on the identifier: Tauri's data, config, local data and cache folders, and the
/// webview's own (WKWebView's under `~/Library`; WebView2's and WebKitGTK's sit in the folders above).
fn pairs(old_id: &str, new_id: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut roots: Vec<(Option<PathBuf>, &str)> = vec![
        (dirs::data_dir(), ""),
        (dirs::config_dir(), ""),
        (dirs::data_local_dir(), ""),
        (dirs::cache_dir(), ""),
    ];
    if cfg!(target_os = "macos") {
        if let Some(lib) = dirs::home_dir().map(|h| h.join("Library")) {
            for sub in ["WebKit", "HTTPStorages", "Logs"] {
                roots.push((Some(lib.join(sub)), ""));
            }
            roots.push((Some(lib.join("Preferences")), ".plist"));
        }
    }
    roots
        .into_iter()
        .filter_map(|(dir, suffix)| {
            let dir = dir?;
            Some((
                dir.join(format!("{old_id}{suffix}")),
                dir.join(format!("{new_id}{suffix}")),
            ))
        })
        .collect()
}

/// The startup move; the lines are for the merged log, which is not open yet.
pub fn run() -> Vec<String> {
    move_all(&pairs(OLD_ID, NEW_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("k7-idmove-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn both_absent_does_nothing() {
        let d = tmp("absent");
        assert_eq!(move_one(&d.join("old"), &d.join("new")), None);
        assert!(!d.join("new").exists());
    }

    #[test]
    fn old_only_is_renamed_with_its_content() {
        let d = tmp("old-only");
        fs::create_dir(d.join("old")).unwrap();
        fs::write(d.join("old/ws.json"), "x").unwrap();
        let line = move_one(&d.join("old"), &d.join("new")).unwrap();
        assert!(line.contains("->") && !line.contains("failed"));
        assert!(!d.join("old").exists());
        assert_eq!(fs::read_to_string(d.join("new/ws.json")).unwrap(), "x");
    }

    #[test]
    fn new_only_is_left_alone() {
        let d = tmp("new-only");
        fs::create_dir(d.join("new")).unwrap();
        assert_eq!(move_one(&d.join("old"), &d.join("new")), None);
        assert!(d.join("new").exists());
    }

    #[test]
    fn both_present_keeps_both_and_names_them() {
        let d = tmp("both");
        fs::create_dir(d.join("old")).unwrap();
        fs::create_dir(d.join("new")).unwrap();
        fs::write(d.join("old/a"), "1").unwrap();
        let line = move_one(&d.join("old"), &d.join("new")).unwrap();
        assert!(line.contains("both kept") && line.contains("old") && line.contains("new"));
        assert!(d.join("old/a").exists() && !d.join("new/a").exists());
    }

    #[test]
    fn a_failed_rename_is_logged_and_keeps_the_old_one() {
        let d = tmp("fail");
        fs::create_dir(d.join("old")).unwrap();
        let new = d.join("missing-parent").join("new");
        let line = move_one(&d.join("old"), &new).unwrap();
        assert!(line.contains("failed"));
        assert!(d.join("old").exists());
    }

    #[test]
    fn a_path_shared_by_two_kinds_moves_once() {
        let d = tmp("dedupe");
        fs::create_dir(d.join("old")).unwrap();
        let pair = (d.join("old"), d.join("new"));
        let lines = move_all(&[pair.clone(), pair]);
        assert_eq!(lines.len(), 1);
        assert!(d.join("new").exists());
    }

    #[test]
    fn the_paths_are_named_after_both_identifiers() {
        for (old, new) in pairs(OLD_ID, NEW_ID) {
            assert!(old.to_string_lossy().contains(OLD_ID));
            assert!(new.to_string_lossy().contains(NEW_ID));
            assert_eq!(old.parent(), new.parent());
        }
    }
}
