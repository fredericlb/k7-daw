//! machine.rs — where the engine binary is (`findEngineBinary`'s tiers), which the renders and the media verbs run.
//! What a reader gets wrong: `STUDIO_ENGINE` wins only when its file is there; else the newest build, Debug last.

use std::path::{Path, PathBuf};

const NAMES: [&str; 2] = ["k7-runtime.exe", "k7-runtime"];

/// `findEngineBinary`: `STUDIO_ENGINE`, then the newest CMake build under the code root's engine, Debug last.
pub fn engine_binary(studio_engine: Option<&str>, code_root: &Path) -> Option<PathBuf> {
    if let Some(p) = studio_engine.filter(|p| !p.is_empty()).map(PathBuf::from)
        && p.exists()
    {
        return Some(p);
    }
    let build = code_root.join("apps/k7-daw-engine/k7-runtime/build");
    let mut found: Vec<(bool, std::time::SystemTime, PathBuf)> = Vec::new();
    for preset in std::fs::read_dir(&build).ok()?.flatten() {
        for cfg in ["", "Release", "RelWithDebInfo", "Debug"] {
            for name in NAMES {
                let p = preset.path().join(cfg).join(name);
                if let Ok(m) = std::fs::metadata(&p)
                    && m.is_file()
                {
                    found.push((
                        cfg == "Debug",
                        m.modified().unwrap_or(std::time::UNIX_EPOCH),
                        p,
                    ));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    found.into_iter().next().map(|f| f.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_named_engine_wins_and_a_missing_one_falls_through() {
        let here = std::env::current_exe().unwrap();
        let none = std::env::temp_dir().join("k7-no-code-root");
        assert_eq!(
            engine_binary(Some(&here.display().to_string()), &none),
            Some(here)
        );
        assert_eq!(engine_binary(Some("/nowhere/k7-runtime"), &none), None);
        assert_eq!(engine_binary(None, &none), None);
    }
}
