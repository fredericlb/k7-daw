//! switch.rs — where the front reads its code root (`K7_CODE_ROOT`, else a checkout above the front), and the one
//! line a host gets for a switch the front no longer reads (C6).
//! What a reader gets wrong: a removed switch never stops the front from starting; it is said once and ignored.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A setting from the host's `Config.env` first, this process's environment else.
fn setting(env: &BTreeMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .cloned()
        .or_else(|| std::env::var(key).ok())
        .filter(|v| !v.trim().is_empty())
}

/// The settings the front no longer reads: every account is the front's, every piece answers, no node child starts, no runtime has a heap (C6).
pub const REMOVED: [&str; 11] = [
    "K7_FILE_SERVICE",
    "K7_FILE_NATIVE",
    "K7_SONG_NATIVE",
    "K7_ACTOR_HEAP_MB",
    "K7_ACTOR_HEAP_LOG",
    "K7_RUNTIME_IDLE_MS",
    "K7_NODE",
    "STUDIO_FILE_SERVICE_ENTRY",
    "STUDIO_LIBRARY_SERVICE_ENTRY",
    "STUDIO_MAX_FILE_SERVICES",
    "STUDIO_FILES_WATCH",
];

/// The one line said where a host still names a removed switch, or `None`; the front starts either way (C6).
pub fn removed_line(env: &BTreeMap<String, String>) -> Option<String> {
    let named: Vec<String> = REMOVED
        .iter()
        .filter_map(|k| setting(env, k).map(|v| format!("{k}={}", v.trim())))
        .collect();
    (!named.is_empty()).then(|| {
        format!(
            "{} no longer read: every account is the front's, every native piece answers and no node service starts",
            named.join(", ")
        )
    })
}

/// What a code root must hold: the files the native pieces read at its root (`scripts/stageCodeRoot.mjs` lists them all).
const MARKS: [&str; 3] = [
    "packages/commands/commands.yaml",
    "packages/song/meta",
    "demos",
];

/// Whether `dir` holds what the native pieces read there.
pub fn is_code_root(dir: &Path) -> bool {
    MARKS.iter().all(|m| dir.join(m).exists())
}

/// The code root: `K7_CODE_ROOT` when it holds the pieces' files, else a checkout above the front.
pub fn code_root(env: &BTreeMap<String, String>) -> Option<PathBuf> {
    if let Some(dir) = setting(env, "K7_CODE_ROOT").map(PathBuf::from) {
        if is_code_root(&dir) {
            return Some(dir);
        }
        eprintln!(
            "k7-front: K7_CODE_ROOT={} holds no code root ({}); looking for a checkout",
            dir.display(),
            MARKS.join(", ")
        );
    }
    let search: Vec<PathBuf> = [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf)),
    ]
    .into_iter()
    .flatten()
    .collect();
    search
        .iter()
        .find_map(|start| start.ancestors().find(|dir| is_code_root(dir)))
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_removed_switch_is_said_in_one_line_and_none_is_silence() {
        assert_eq!(removed_line(&BTreeMap::new()), None);
        for k in REMOVED {
            let env = BTreeMap::from([(k.to_string(), "off".to_string())]);
            let line = removed_line(&env).unwrap();
            assert!(line.contains(&format!("{k}=off")), "{line}");
        }
        let all: BTreeMap<String, String> = REMOVED
            .iter()
            .map(|k| (k.to_string(), "x".to_string()))
            .collect();
        assert_eq!(
            removed_line(&all).unwrap().matches("=x").count(),
            REMOVED.len()
        );
    }

    #[test]
    fn a_named_code_root_wins_when_it_holds_the_pieces_files() {
        let dir = std::env::temp_dir().join(format!("k7-code-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("packages/commands")).unwrap();
        let env = BTreeMap::from([("K7_CODE_ROOT".to_string(), dir.display().to_string())]);
        assert!(!is_code_root(&dir));
        assert_ne!(code_root(&env).as_deref(), Some(dir.as_path()));
        std::fs::write(dir.join("packages/commands/commands.yaml"), "").unwrap();
        std::fs::create_dir_all(dir.join("packages/song/meta")).unwrap();
        std::fs::create_dir_all(dir.join("demos")).unwrap();
        assert_eq!(code_root(&env).as_deref(), Some(dir.as_path()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_checkout_is_a_code_root() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
        assert!(is_code_root(&repo));
    }
}
