//! A workspace made and removed (1660 to 1662): the plan's verdicts, the folder made whole, the folder and its marker
//! removed, a link not followed, a folder refused its removal; what Windows answers differently is guarded in the test.

use crate::support;

use k7_file_service::space::Declined;
use k7_file_service::workspaces::WsError;
use k7_file_service::workspaces::life::*;
use support::ws::Acct;
use support::{can_link, link};

fn exists(a: &Acct, rel: &str) -> bool {
    std::fs::symlink_metadata(a.s.path(rel)).is_ok()
}

#[test]
fn a_new_name_is_made_whole_and_a_second_make_is_false() {
    let a = Acct::new("ws-life-make");
    let h = a.home();
    assert_eq!(plan_make(&h, false, "songs"), Ok(true));
    assert!(
        !exists(&a, "studio/workspace/songs"),
        "a plan creates nothing"
    );
    assert_eq!(make(&h, "songs"), Ok(true));
    assert!(exists(&a, "studio/workspace/songs"));
    assert_eq!(plan_make(&h, false, "songs"), Ok(false));
    assert_eq!(make(&h, "songs"), Ok(false));
}

#[test]
fn the_parent_folders_are_made_too() {
    let a = support::Scratch::new("ws-life-parents");
    a.dir("libraries/k7-core");
    let h = a.home(true);
    assert_eq!(make(&h, "first"), Ok(true));
    assert!(std::fs::metadata(a.path("studio/workspace/first")).is_ok());
    assert_eq!(plan_make(&h, true, "second"), Ok(true));
}

#[test]
fn a_name_the_allow_list_refuses_is_invalid() {
    let a = Acct::new("ws-life-invalid");
    for bad in ["", ".", "..", "a/b", " a", "a\\b", "é", "-x"] {
        assert_eq!(
            plan_make(&a.home(), false, bad),
            Err(WsError::Invalid { name: bad.into() }),
            "{bad:?}"
        );
        assert_eq!(
            make(&a.home(), bad),
            Err(WsError::Invalid { name: bad.into() })
        );
        assert_eq!(
            plan_remove(&a.home(), bad),
            Err(WsError::Invalid { name: bad.into() })
        );
    }
    assert_eq!(
        WsError::Invalid { name: "..".into() }.to_string(),
        "invalid workspace name: .."
    );
}

#[test]
fn with_no_library_on_disk_the_make_is_declined_before_the_name_is_judged() {
    let s = support::Scratch::new("ws-life-nolib");
    let h = s.home(false);
    assert_eq!(
        plan_make(&h, false, "songs"),
        Err(WsError::Declined(Declined::NoLibraries))
    );
    assert_eq!(
        plan_make(&h, false, ".."),
        Err(WsError::Declined(Declined::NoLibraries))
    );
    // A file, or a name no library may have, is not a library.
    s.file("libraries/afile", "x");
    s.dir("libraries/Not-A-Name");
    s.dir("libraries/engine");
    assert_eq!(
        plan_make(&h, false, "songs"),
        Err(WsError::Declined(Declined::NoLibraries))
    );
    s.dir("libraries/k7-core");
    assert_eq!(plan_make(&h, false, "songs"), Ok(true));
}

#[test]
fn a_make_of_the_other_case_is_what_the_disk_says() {
    let a = Acct::new("ws-life-case");
    make(&a.home(), "Songs").unwrap();
    let folds = std::fs::metadata(a.s.path("studio/workspace/songs")).is_ok();
    assert_eq!(plan_make(&a.home(), false, "songs"), Ok(!folds));
}

#[test]
fn a_trailing_dot_is_a_name_and_windows_drops_it() {
    let a = Acct::new("ws-life-dot");
    assert_eq!(make(&a.home(), "ws."), Ok(true));
    // Windows makes `ws` and answers `created: true`; elsewhere `ws.` is a folder of its own.
    assert!(exists(
        &a,
        if cfg!(windows) {
            "studio/workspace/ws"
        } else {
            "studio/workspace/ws."
        }
    ));
}

#[test]
fn a_removal_names_the_folder_in_its_own_spelling() {
    let a = Acct::new("ws-life-plan");
    a.ws("Songs");
    a.ws("Examples");
    a.ws("Banc");
    let h = a.home();
    let root = |n: &str| a.s.path(&format!("studio/workspace/{n}"));
    assert_eq!(plan_remove(&h, "Songs"), Ok(root("Songs")));
    assert_eq!(plan_remove(&h, "Examples"), Ok(root("Examples")));
    assert_eq!(plan_remove(&h, "Banc"), Ok(root("Banc")));
    assert_eq!(
        plan_remove(&h, "absent"),
        Err(WsError::NoSuch {
            name: "absent".into()
        })
    );
    assert_eq!(
        WsError::NoSuch {
            name: "absent".into()
        }
        .to_string(),
        "no such workspace: absent"
    );
    // On a disk that folds case the other spelling is the folder; elsewhere there is none (1632).
    let folds = std::fs::metadata(root("songs")).is_ok();
    let want = if folds {
        Ok(root("Songs"))
    } else {
        Err(WsError::NoSuch {
            name: "songs".into(),
        })
    };
    assert_eq!(plan_remove(&h, "songs"), want);
}

#[test]
fn a_removal_takes_the_folder_whole_and_its_marker_and_nothing_else() {
    let a = Acct::new("ws-life-remove");
    a.project("songs", "winter.k7", "project({})\n");
    a.project("albums", "other.k7", "project({})\n");
    a.marker("songs", "winter.k7");
    a.marker("albums", "other.k7");
    let root = plan_remove(&a.home(), "songs").unwrap();
    assert_eq!(remove(&a.home(), &root), Ok(()));
    assert!(!exists(&a, "studio/workspace/songs"));
    assert!(!exists(&a, "studio/workspace/.active-projects/songs"));
    assert!(exists(&a, "studio/workspace/albums/other.k7/project.k7x"));
    assert!(exists(&a, "studio/workspace/.active-projects/albums"));
    // Again: nothing is there, which is not an error (`force`).
    assert_eq!(remove(&a.home(), &root), Ok(()));
}

#[test]
fn a_workspace_that_is_a_link_is_unlinked_and_its_target_kept() {
    let a = Acct::new("ws-life-link");
    if !can_link(&a.s) {
        return;
    }
    let target = a.s.dir("elsewhere");
    a.s.file("elsewhere/keep.txt", "kept");
    link(&target, &a.s.path("studio/workspace/linked")).unwrap();
    assert_eq!(
        remove(&a.home(), &a.s.path("studio/workspace/linked")),
        Ok(())
    );
    assert!(!exists(&a, "studio/workspace/linked"));
    assert!(exists(&a, "elsewhere/keep.txt"));
}

#[test]
fn a_junction_is_removed_as_a_folder() {
    // A Windows junction reads as a link to `readdir` and as a folder to the file API; elsewhere a link is a link.
    let a = Acct::new("ws-life-junction");
    if !can_link(&a.s) {
        return;
    }
    let target = a.s.dir("inner/target");
    link(&target, &a.s.path("studio/workspace/j")).unwrap();
    assert_eq!(remove(&a.home(), &a.s.path("studio/workspace/j")), Ok(()));
    assert!(exists(&a, "inner/target"));
}

#[cfg(windows)]
#[test]
fn a_removal_under_a_held_handle_answers_its_code() {
    use std::os::windows::fs::OpenOptionsExt;
    let a = Acct::new("ws-life-held");
    let file = a.s.file("studio/workspace/held/f.txt", "x");
    let _open = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&file)
        .unwrap();
    match remove(&a.home(), &a.s.path("studio/workspace/held")) {
        Err(WsError::Io { code, syscall, .. }) => {
            assert!(matches!(code, "EBUSY" | "EACCES"), "{code}");
            assert_eq!(syscall, "rm");
        }
        other => panic!("{other:?}"),
    }
    assert!(
        exists(&a, "studio/workspace/held"),
        "what could not go stays"
    );
}

#[cfg(not(windows))]
#[test]
fn a_removal_under_a_handle_open_elsewhere_just_removes() {
    let a = Acct::new("ws-life-held");
    let file = a.s.file("studio/workspace/held/f.txt", "x");
    let _open = std::fs::File::open(&file).unwrap();
    assert_eq!(
        remove(&a.home(), &a.s.path("studio/workspace/held")),
        Ok(())
    );
    assert!(!exists(&a, "studio/workspace/held"));
}

#[cfg(unix)]
#[test]
fn a_folder_refused_its_removal_answers_the_code_and_the_path() {
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let a = Acct::new("ws-life-refused");
    a.s.file("studio/workspace/locked/inner/f.txt", "x");
    let inner = a.s.path("studio/workspace/locked/inner");
    std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o500)).unwrap();
    let r = remove(&a.home(), &a.s.path("studio/workspace/locked"));
    std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o755)).unwrap();
    match r {
        Err(e @ WsError::Io { .. }) => {
            assert_eq!(e.code(), Some("EACCES"));
            assert!(
                e.to_string().starts_with("EACCES: permission denied, rm '"),
                "{e}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_workspace_of_many_files_is_removed() {
    let a = Acct::new("ws-life-many");
    let root = a.ws("big");
    for d in 0..20 {
        std::fs::create_dir_all(format!("{root}/d{d}")).unwrap();
        for f in 0..500 {
            std::fs::write(format!("{root}/d{d}/f{f}.txt"), "x").unwrap();
        }
    }
    // Not timed: deleting 10,000 files on a loaded disk says nothing of a hang, and `remove_dir_all` follows no link.
    assert_eq!(remove(&a.home(), &root), Ok(()));
    assert!(!exists(&a, "studio/workspace/big"));
}
