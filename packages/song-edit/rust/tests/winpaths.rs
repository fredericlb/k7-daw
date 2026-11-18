// winpaths.rs — Windows-shaped paths through the posix algebra and the confinement; every case runs on every OS.
// A reader would get wrong that `/C:/x` is the crate's spelling of `C:\x`: only the host edge (`winpath`) knows the backslash.

use song_edit::confine::{Env, confine_to, confine_to_project};
use song_edit::mem_fs::MemFs;
use song_edit::paths::{is_within, relative, resolve};
use song_edit::winpath::{from_posix, to_posix, user_path};

fn fs() -> MemFs {
    MemFs::from_tree(
        "/C:/studio",
        [("/C:/studio/workspace/p/a.k7x".to_owned(), b"x".to_vec())],
    )
}

fn env() -> Env {
    Env::at("/C:/studio", "/C:/studio/meta")
}

fn project(reference: &str) -> Option<String> {
    confine_to_project(&fs(), &env(), "/C:/studio/workspace/p", reference, None)
}

#[test]
fn real_paths_map_to_posix_and_back() {
    for (real, posix) in [
        ("C:\\studio\\x", "/C:/studio/x"),
        ("c:/studio/x", "/C:/studio/x"),
        ("\\\\?\\C:\\studio\\x", "/C:/studio/x"),
        ("\\\\srv\\share\\d\\x", "/UNC:/srv/share/d/x"),
        ("\\\\?\\UNC\\srv\\share\\x", "/UNC:/srv/share/x"),
        ("/home/me/x", "/home/me/x"),
    ] {
        assert_eq!(to_posix(real).as_deref(), Some(posix), "{real}");
    }
    assert_eq!(from_posix("/C:/studio/x"), "C:\\studio\\x");
    assert_eq!(from_posix("/C:"), "C:\\");
    assert_eq!(from_posix("/UNC:/srv/share/x"), "\\\\srv\\share\\x");
    assert_eq!(from_posix("/w/x"), "/w/x");
}

#[test]
fn device_and_drive_relative_paths_are_refused() {
    for bad in ["\\\\.\\COM1", "C:x", "C:..\\x", "rel\\x"] {
        assert_eq!(to_posix(bad), None, "{bad}");
    }
}

#[test]
fn dotdot_never_climbs_past_a_drive_or_a_share() {
    assert_eq!(resolve("/C:/../../x"), "/C:/x");
    assert_eq!(resolve("/C:/a/../../D:/x"), "/C:/D:/x");
    assert_eq!(resolve("/UNC:/srv/share/../../x"), "/UNC:/srv/share/x");
    assert_eq!(resolve("/c:/a"), "/C:/a");
}

#[test]
fn windows_names_compare_without_case() {
    assert!(is_within("/C:/Studio", "/C:/studio/Workspace/p"));
    assert!(is_within("/UNC:/Srv/Share", "/UNC:/srv/share/x"));
    assert!(!is_within("/C:/studio", "/D:/studio/x"));
    assert!(!is_within("/C:/studio", "/C:/studio2"));
    assert_eq!(relative("/C:/Studio", "/C:/studio/a"), "a");
}

#[test]
fn posix_names_keep_their_case() {
    assert!(!is_within("/w/Studio", "/w/studio/x"));
}

#[test]
fn confinement_accepts_the_project_in_any_spelling() {
    assert_eq!(project("a.k7x").as_deref(), Some("/C:/studio/workspace/p/a.k7x"));
    assert!(project("sub\\a.k7x").is_some());
    assert!(project("C:\\Studio\\Workspace\\p\\a.k7x").is_some());
    assert!(project("\\\\?\\c:\\STUDIO\\workspace\\p\\a.k7x").is_some());
}

#[test]
fn confinement_refuses_every_escape() {
    for bad in [
        "..\\..\\..\\..\\x",
        "../../../../x",
        "..\\other\\x",
        "D:\\studio\\workspace\\p\\a.k7x",
        "D:x",
        "C:x",
        "\\\\srv\\share\\x",
        "\\\\?\\UNC\\srv\\share\\x",
        "\\\\?\\D:\\x",
        "\\\\.\\C:\\studio\\x",
        "\\x",
        "C:\\studio\\workspace\\p\\..\\..\\..\\x",
        "a.k7x:stream",
        "C:\\studio\\workspace\\p\\a.k7x:stream",
    ] {
        assert_eq!(project(bad), None, "{bad}");
    }
}

#[test]
fn a_unc_root_confines_its_own_share_only() {
    let roots = vec!["/UNC:/srv/share/lib".to_owned()];
    let fs = MemFs::new();
    assert!(confine_to(&fs, &roots, "/UNC:/SRV/share/lib/x").is_some());
    assert!(confine_to(&fs, &roots, "/UNC:/srv/other/lib/x").is_none());
    assert!(confine_to(&fs, &roots, "/UNC:/srv/share/lib/../x").is_none());
    assert!(confine_to(&fs, &roots, "/C:/srv/share/lib/x").is_none());
}

#[test]
fn user_paths_read_backslashes_as_separators() {
    assert_eq!(user_path("a\\b").as_deref(), Some("a/b"));
    assert_eq!(user_path("C:\\a").as_deref(), Some("/C:/a"));
    assert_eq!(user_path("C:a"), None);
}

#[cfg(windows)]
#[test]
fn the_real_temp_dir_round_trips() {
    let tmp = std::env::temp_dir();
    let posix = to_posix(tmp.to_str().unwrap()).unwrap();
    assert!(posix.starts_with("/C:/") || posix.starts_with("/UNC:/") || posix.len() > 3);
    assert_eq!(to_posix(&from_posix(&posix)).as_deref(), Some(posix.as_str()));
}

#[test]
fn a_library_module_under_a_drive_is_in_the_vocabulary_on_every_host() {
    let fs = MemFs::from_tree(
        "/C:/studio",
        [
            (
                "libraries/k7-core/modules/audio/eq/module.k7x".to_owned(),
                b"extends(\"@engine/reverb\")\n".to_vec(),
            ),
            ("workspace/p/project.k7x".to_owned(), b"project({})\n".to_vec()),
        ],
    );
    let found = song_edit::loader::load_module_refs(&fs, &env(), "/C:/studio/workspace/p").unwrap();
    assert!(found.modules.contains_key("@k7-core/audio/eq"), "{:?}", found.findings);
}
