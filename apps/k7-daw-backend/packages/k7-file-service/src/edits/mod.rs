//! edits — the server's own edits of a person's files, through song-edit's `Fs`: staged in memory and landed whole, or written at once.
//! What a reader gets wrong: `StagedFs` is the rule (one commit path, 3d's); `DiskFs` is for the bench's own folder and the stamp only.

pub mod disk_fs;
pub mod edge;
pub mod import;
pub mod module_make;
pub mod modules;
pub mod open;
pub mod picture;
pub mod prepare;
pub mod presets;
pub mod project_create;
pub mod staged;
pub mod stamp;
pub mod take;
pub mod verb;

pub use disk_fs::DiskFs;
pub use staged::StagedFs;

use song_edit::confine::Env;
use song_edit::paths::join;

use crate::libraries::{Home, library_root};
use crate::listings::families::library_names;

/// The roots song-edit reads from: the account's libraries and the engine's meta modules.
pub fn env_of(home: &Home, code_root: &str) -> Env {
    // A code root given as a relative path is made absolute: a mount must be.
    let code = if song_edit::winpath::to_posix(code_root).is_some() {
        code_root.to_owned()
    } else {
        std::path::absolute(code_root)
            .map_or_else(|_| code_root.to_owned(), |p| p.display().to_string())
    };
    // Resolved in the host's own spelling first: song-edit's join splits on '/' alone and would keep a `..` after a backslash.
    let code = crate::paths::resolve_all(&[&code]);
    let mut env = Env::at(
        &posix(&home.studio),
        &posix(&join(&[&code, "apps/k7-daw-wasm/song/meta"])),
    );
    env.libraries_home.clone_from(&home.libraries);
    edge::posix_env(env)
}

/// `env_of` with every library read from the folder the project's workspace pins; an unreadable pin keeps each library's own folder.
pub fn env_for(home: &Home, code_root: &str, project: &str) -> Env {
    let mut env = env_of(home, code_root);
    let roots: Option<Vec<(String, String)>> = library_names(home)
        .into_iter()
        .map(|n| library_root(home, &n, Some(project)).ok().map(|r| (n, r)))
        .collect();
    env.library_roots = roots;
    edge::posix_env(env)
}

/// A real path (`C:\x` on Windows) in the posix form song-edit computes on; a posix path comes back as it is.
#[must_use]
pub fn posix(real: &str) -> String {
    song_edit::winpath::to_posix(real).unwrap_or_else(|| song_edit::paths::resolve(real))
}

/// The real path of a posix-form one (`/C:/x` is `C:\x`); std adds the verbatim prefix a long path needs.
#[must_use]
pub fn real(p: &str) -> String {
    song_edit::winpath::from_posix(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Windows-shaped roots go in as posix and come out as the real path, never with a hand-made prefix.
    // A Home spelled `C:\\...` reaches song-edit as posix roots, so a pinned library's file is inside them.
    #[test]
    fn a_windows_home_reaches_song_edit_in_posix_roots() {
        let home = Home {
            studio: "C:\\Users\\a\\.k7".into(),
            libraries: "C:\\Users\\a\\.k7\\libraries".into(),
        };
        let env = env_of(&home, "C:\\code");
        assert_eq!(env.studio_home, "/C:/Users/a/.k7");
        assert_eq!(env.libraries_home, "/C:/Users/a/.k7/libraries");
        assert_eq!(env.meta_root, "/C:/code/apps/k7-daw-wasm/song/meta");
        assert!(env.models_root.starts_with("/C:/Users/a/.k7"));
    }

    // The std-side writes of a created project and a frozen chain reach std in the real spelling, lexical `C:/x` and UNC too.
    #[test]
    fn a_lexical_windows_or_unc_path_reaches_std_in_its_real_spelling() {
        let at = |p: &str| real(&posix(p));
        assert_eq!(
            at("C:/Users/a/p.k7/clips/sample.mp4"),
            "C:\\Users\\a\\p.k7\\clips\\sample.mp4"
        );
        assert_eq!(
            at("//srv/share/vst/1.vstpreset"),
            "\\\\srv\\share\\vst\\1.vstpreset"
        );
        assert_eq!(at("/tmp/x/chains/a.k7x"), "/tmp/x/chains/a.k7x");
    }

    #[test]
    fn a_windows_path_crosses_the_edge_both_ways() {
        assert_eq!(posix("C:\\Users\\a\\p.k7"), "/C:/Users/a/p.k7");
        assert_eq!(real("/C:/Users/a/p.k7"), "C:\\Users\\a\\p.k7");
        let long = format!("/C:/{}", "d/".repeat(140));
        assert!(real(&long).starts_with("C:\\d\\"));
        assert_eq!(real("/tmp/x"), "/tmp/x");
        assert_eq!(posix("/tmp/x"), "/tmp/x");
    }
}
