//! `plan` for each of the five writes: every vitest case of the piece (`moveEntry`, `createFolder`, `libraryEntries`,
//! `sharedModules`, `entryErrors`), the sentences, the order of the guards, and a `C:/` workspace on any host.

use crate::support;

use k7_file_service::entries::{EntryError, Outcome};
use k7_file_service::space::PathError;
use support::entries::{Fx, delete, folder, mv, rename, said, upload};

#[test]
fn a_folder_is_made_where_it_is_asked_and_takes_one_segment() {
    let fx = Fx::new("p-folder");
    assert_eq!(
        said(&fx, folder("song.k7/clips", "takes2")),
        serde_json::json!({ "path": "song.k7/clips/takes2" })
    );
    assert_eq!(
        fx.user(folder("song.k7/clips", "takes2")),
        Ok(Outcome::Folder {
            abs: fx.at("song.k7/clips/takes2")
        })
    );
    // `basename` reduces rather than rejects: nothing lands outside the clicked folder.
    assert_eq!(
        said(&fx, folder("song.k7/clips", "../../evil")),
        serde_json::json!({ "path": "song.k7/clips/evil" })
    );
    assert_eq!(
        said(&fx, folder("song.k7/clips", "a/inner")),
        serde_json::json!({ "path": "song.k7/clips/inner" })
    );
    assert_eq!(
        said(&fx, folder(".", "top")),
        serde_json::json!({ "path": "top" })
    );
    assert_eq!(
        said(&fx, folder("/", "top")),
        serde_json::json!({ "path": "top" })
    );
    assert_eq!(
        said(&fx, folder("song.k7/clips///", "t")),
        serde_json::json!({ "path": "song.k7/clips/t" })
    );
    // A folder not there yet is made whole.
    assert_eq!(
        said(&fx, folder("no/such", "x")),
        serde_json::json!({ "path": "no/such/x" })
    );
    // The name asked, not the name the disk would make (1236).
    assert_eq!(
        said(&fx, folder(".", "x.")),
        serde_json::json!({ "path": "x." })
    );
    assert_eq!(
        said(&fx, folder(".", "x ")),
        serde_json::json!({ "path": "x" })
    );
}

#[test]
fn a_folder_refuses_a_bad_name_a_bin_a_shared_module_and_a_clobber() {
    let fx = Fx::new("p-folder-no");
    for bad in [
        "..", ".", "   ", "", "  .. ", "\u{feff}", "\u{3000}", "a/..",
    ] {
        let want = format!("invalid folder name: {bad}");
        assert_eq!(fx.refusal(folder("song.k7/clips", bad)), want, "{bad:?}");
    }
    fx.s.dir("studio/workspace/songs/_recycle_bin");
    assert_eq!(
        fx.refusal(folder("_recycle_bin", "nope")),
        "can't create a folder in the recycle bin"
    );
    assert!(
        fx.refusal(folder("_modules/audio", "mine"))
            .contains("a shared module is edited in place")
    );
    assert_eq!(
        fx.user(folder("song.k7/clips", "takes")),
        Err(EntryError::Exists {
            rel: "song.k7/clips/takes".into()
        })
    );
    assert_eq!(
        fx.refusal(folder("song.k7/clips", "takes")),
        "already exists: song.k7/clips/takes"
    );
    assert_eq!(
        fx.refusal(folder("../outside", "x")),
        "path escapes the workspace: ../outside/x"
    );
    // A library: no account may.
    let at = "_libraries/k7-core/modules/audio";
    assert_eq!(
        fx.refusal(folder(at, "new")),
        "can't create a folder in '_libraries/k7-core/modules/audio/new': a library is not edited from here"
    );
}

#[test]
fn a_rename_keeps_its_parent_takes_one_segment_and_says_what_it_did() {
    let fx = Fx::new("p-rename");
    fx.s.file("studio/workspace/songs/a.txt", "a");
    fx.s.file("studio/workspace/songs/b.txt", "b");
    assert_eq!(
        fx.user(rename("a.txt", "z.txt")),
        Ok(Outcome::Rename {
            from: fx.at("a.txt"),
            to: fx.at("z.txt"),
            refs: None
        })
    );
    assert_eq!(
        said(&fx, rename("song.k7/clips", "cuts")),
        serde_json::json!({ "path": "song.k7/cuts" })
    );
    assert_eq!(
        fx.user(rename("a.txt", "a.txt")),
        Ok(Outcome::Unchanged {
            abs: fx.at("a.txt")
        })
    );
    assert_eq!(
        fx.user(rename("a.txt", "  a.txt  ")),
        Ok(Outcome::Unchanged {
            abs: fx.at("a.txt")
        })
    );
    assert_eq!(
        fx.refusal(rename("a.txt", "b.txt")),
        "already exists: b.txt"
    );
    assert_eq!(
        said(&fx, rename("a.txt", "../up.txt")),
        serde_json::json!({ "path": "up.txt" })
    );
    assert_eq!(
        fx.refusal(rename("a.txt", "a.YML")),
        "a.YML: a workspace file is .k7x, not YAML"
    );
    for bad in ["..", ".", "  ", "/", "\u{feff}"] {
        assert_eq!(
            fx.refusal(rename("a.txt", bad)),
            format!("invalid name: {bad}"),
            "{bad:?}"
        );
    }
    assert_eq!(
        fx.user(rename("none.txt", "z.txt")),
        Err(EntryError::NotFound {
            rel: "none.txt".into()
        })
    );
    assert_eq!(
        fx.refusal(rename("none.txt", "z.txt")),
        "no such file or folder: none.txt"
    );
}

#[test]
fn a_rename_that_changes_the_case_alone_is_what_the_disk_makes_of_it() {
    let fx = Fx::new("p-case");
    fx.s.file("studio/workspace/songs/Case.txt", "x");
    // 1237: `exists` asks the disk, so a case-folding disk answers `already exists`.
    let folds = std::fs::metadata(fx.at("case.txt")).is_ok();
    let got = fx.user(rename("Case.txt", "case.txt"));
    if folds {
        assert_eq!(
            got,
            Err(EntryError::Exists {
                rel: "case.txt".into()
            })
        );
    } else {
        assert_eq!(
            got,
            Ok(Outcome::Rename {
                from: fx.at("Case.txt"),
                to: fx.at("case.txt"),
                refs: None
            })
        );
    }
}

#[test]
fn what_a_rename_may_not_touch_is_refused_in_words() {
    let fx = Fx::new("p-rename-no");
    fx.s.file("studio/workspace/songs/_recycle_bin/old/x", "x");
    fx.s.dir("studio/workspace/songs/band/song2.k7");
    assert_eq!(
        fx.refusal(rename(".", "x")),
        "can't rename the workspace root"
    );
    assert_eq!(
        fx.refusal(rename("_recycle_bin", "x")),
        "'_recycle_bin' is already in the recycle bin"
    );
    assert_eq!(
        fx.refusal(rename("_recycle_bin/old", "x")),
        "'_recycle_bin/old' is already in the recycle bin"
    );
    assert_eq!(
        fx.refusal(rename("song.k7", "x")),
        "can't rename the active project's folder — stop it first, then rename it"
    );
    assert_eq!(
        fx.refusal(rename("_libraries", "x")),
        "can't rename '../../../libraries': the libraries are listed, not moved from a workspace"
    );
    assert_eq!(
        fx.refusal(rename("_libraries/k7-core", "x")),
        "can't rename '_libraries/k7-core': the libraries are listed, not moved from a workspace"
    );
    assert!(
        fx.refusal(rename("_modules", "x"))
            .contains("a shared module is edited in place")
    );
    // Inside the active project everything is fair game.
    assert!(
        fx.user(rename("song.k7/clips/riff.k7x", "riff2.k7x"))
            .is_ok()
    );
    // A path that is no library.
    assert_eq!(
        fx.refusal(rename("_libraries/Nobody/x", "y")),
        "names no library: _libraries/Nobody/x"
    );
}

#[test]
fn a_move_goes_into_a_folder_and_refuses_what_it_cannot() {
    let fx = Fx::new("p-move");
    assert_eq!(
        fx.user(mv("song.k7/clips/riff.k7x", "song.k7/sequences")),
        Ok(Outcome::Move {
            from: fx.at("song.k7/clips/riff.k7x"),
            to: fx.at("song.k7/sequences/riff.k7x")
        })
    );
    assert_eq!(
        said(&fx, mv("song.k7/clips/takes", "song.k7/sequences")),
        serde_json::json!({ "path": "song.k7/sequences/takes" })
    );
    // Onto the folder it sits in, the same answer as a rename to its own name.
    assert_eq!(
        fx.user(mv("song.k7/clips/riff.k7x", "song.k7/clips")),
        Ok(Outcome::Unchanged {
            abs: fx.at("song.k7/clips/riff.k7x")
        })
    );
    assert_eq!(
        fx.refusal(mv("song.k7/clips", "song.k7/clips/takes")),
        "can't move 'clips' into itself"
    );
    fx.s.file("studio/workspace/songs/song.k7/sequences/riff.k7x", "");
    assert_eq!(
        fx.refusal(mv("song.k7/clips/riff.k7x", "song.k7/sequences")),
        "already exists: song.k7/sequences/riff.k7x"
    );
    assert_eq!(
        fx.refusal(mv("song.k7/clips/takes", "song.k7/clips/riff.k7x")),
        "not a folder: song.k7/clips/riff.k7x"
    );
    assert_eq!(
        fx.refusal(mv("song.k7/clips/takes", "song.k7/nope")),
        "not a folder: song.k7/nope"
    );
    fx.s.dir("studio/workspace/songs/elsewhere");
    assert_eq!(
        fx.refusal(mv("song.k7", "elsewhere")),
        "can't move the active project's folder — stop it first, then move it"
    );
    fx.s.dir("studio/workspace/songs/_recycle_bin");
    assert_eq!(
        fx.refusal(mv("song.k7/clips/riff.k7x", "_recycle_bin")),
        "can't move something into the recycle bin — delete it instead"
    );
    assert_eq!(
        fx.refusal(mv("song.k7/clips/riff.k7x", "../")),
        "path escapes the workspace: ../"
    );
    assert_eq!(
        fx.user(mv("none", "elsewhere")),
        Err(EntryError::NotFound { rel: "none".into() })
    );
}

#[test]
fn three_hundred_deletes_of_one_name_have_no_bound() {
    let fx = Fx::new("p-delete-300");
    fx.s.file("studio/workspace/songs/x", "x");
    fx.s.file("studio/workspace/songs/_recycle_bin/x", "");
    for n in 2..=300 {
        fx.s.file(&format!("studio/workspace/songs/_recycle_bin/x-{n}"), "");
    }
    assert_eq!(
        said(&fx, delete("x")),
        serde_json::json!({ "recycled": "_recycle_bin/x-301" })
    );
}

#[test]
fn what_a_delete_may_not_touch() {
    let fx = Fx::new("p-delete-no");
    fx.s.file("studio/workspace/songs/_recycle_bin/old", "x");
    assert_eq!(fx.refusal(delete(".")), "can't delete the workspace root");
    assert_eq!(
        fx.refusal(delete("_recycle_bin")),
        "'_recycle_bin' is already in the recycle bin"
    );
    assert_eq!(
        fx.refusal(delete("_recycle_bin/old")),
        "'_recycle_bin/old' is already in the recycle bin"
    );
    assert_eq!(
        fx.refusal(delete("song.k7")),
        "can't delete the active project's folder — stop it first, then delete it"
    );
    assert_eq!(
        fx.refusal(delete("_libraries")),
        "can't delete '../../../libraries': the libraries are listed, not moved from a workspace"
    );
    assert!(
        fx.refusal(delete("_modules/audio"))
            .contains("module populate")
    );
}

#[test]
fn an_upload_is_judged_by_its_trimmed_segments_and_its_rel() {
    let fx = Fx::new("p-upload");
    assert_eq!(
        fx.user(upload(".", "a.wav")),
        Ok(Outcome::Upload {
            abs: fx.at("a.wav")
        })
    );
    assert_eq!(
        said(&fx, upload("media", "take/one.bin")),
        serde_json::json!({ "path": "media/take/one.bin" })
    );
    assert_eq!(
        said(&fx, upload("media/", " a / b .txt")),
        serde_json::json!({ "path": "media/a/b .txt" })
    );
    assert_eq!(
        said(&fx, upload(" sub ", "x.bin")),
        serde_json::json!({ "path": " sub /x.bin" })
    );
    assert_eq!(
        said(&fx, upload("", "x\\y\\z.bin")),
        serde_json::json!({ "path": "x/y/z.bin" })
    );
    for bad in ["../e.bin", "a/../b", "..", ".", "  ", "/", "a/./b"] {
        assert_eq!(
            fx.refusal(upload(".", bad)),
            format!("invalid filename: {bad}"),
            "{bad:?}"
        );
    }
    assert_eq!(
        fx.refusal(upload(".", "b.yaml")),
        "b.yaml: a workspace file is .k7x, not YAML"
    );
    assert_eq!(
        fx.refusal(upload("song.k7", "x.YML")),
        "song.k7/x.YML: a workspace file is .k7x, not YAML"
    );
    fx.s.file("studio/workspace/songs/there.bin", "x");
    assert_eq!(
        fx.user(upload(".", "there.bin")),
        Err(EntryError::Exists {
            rel: "there.bin".into()
        })
    );
    assert!(
        fx.refusal(upload("_modules", "ir.wav"))
            .contains("can't add a file to '_modules/ir.wav'")
    );
    assert_eq!(
        fx.refusal(upload("_libraries/k7-core/modules/audio/verb", "ir.wav")),
        "can't add a file to '_libraries/k7-core/modules/audio/verb/ir.wav': a library is not edited from here"
    );
}

#[test]
fn a_gate_refusal_is_the_gate_s_sentence_and_a_long_path_is_declined() {
    let fx = Fx::new("p-gate");
    assert_eq!(
        fx.user(rename("../x", "y")),
        Err(EntryError::Path(PathError::Escapes {
            what: "workspace",
            rel: "../x".into()
        }))
    );
    let long = "a".repeat(16 * 1024 + 1);
    assert_eq!(
        fx.user(upload(".", &long)),
        Err(EntryError::Declined(
            k7_file_service::space::Declined::LongPath
        ))
    );
    assert_eq!(
        fx.user(folder(".", &long)),
        Err(EntryError::Declined(
            k7_file_service::space::Declined::LongPath
        ))
    );
}
