//! `plan` where a disk has more to say: a link renamed and recycled as itself, a dangling one the gate refuses, and a
//! `C:/` workspace spelt on any host (1222, 1221).

use crate::support;

use k7_file_service::entries::plan::plan;
use k7_file_service::entries::{EntryError, Op, Outcome};
use k7_file_service::rel::to_rel;
use k7_file_service::space::Space;
use support::entries::{Drive, Fx, delete, drive_home, folder, mv, rename, upload};
use support::{can_link, link, link_file};

#[test]
fn a_link_is_renamed_and_recycled_as_itself_and_a_dangling_one_is_refused_by_the_gate() {
    let fx = Fx::new("p-link");
    if !can_link(&fx.s) {
        return;
    }
    fx.s.file("studio/workspace/songs/target.txt", "t");
    fx.s.dir("studio/workspace/songs/tdir");
    link_file(&fx.at("target.txt"), &fx.at("lnk.txt")).unwrap();
    link(&fx.at("tdir"), &fx.at("ldir")).unwrap();
    assert_eq!(
        fx.user(rename("lnk.txt", "l2.txt")),
        Ok(Outcome::Rename {
            from: fx.at("lnk.txt"),
            to: fx.at("l2.txt"),
            refs: None
        })
    );
    assert_eq!(
        fx.user(mv("lnk.txt", "tdir")),
        Ok(Outcome::Move {
            from: fx.at("lnk.txt"),
            to: fx.at("tdir/lnk.txt")
        })
    );
    // A move into a link to a folder lands in what it points at, by the path it was named by.
    assert_eq!(
        fx.user(mv("lnk.txt", "ldir")),
        Ok(Outcome::Move {
            from: fx.at("lnk.txt"),
            to: fx.at("ldir/lnk.txt")
        })
    );
    assert_eq!(
        fx.user(delete("lnk.txt")),
        Ok(Outcome::Recycle {
            from: fx.at("lnk.txt"),
            to: fx.at("_recycle_bin/lnk.txt")
        })
    );
    // A link to nothing: the gate says it escapes the workspace (measured).
    let ghost = fx.s.path("ghost-target");
    link_file(&ghost, &fx.at("dangling")).unwrap();
    assert_eq!(
        fx.refusal(false, rename("dangling", "d2")),
        "path escapes the workspace: dangling"
    );
    assert_eq!(
        fx.refusal(false, delete("dangling")),
        "path escapes the workspace: dangling"
    );
}

#[test]
fn a_dangling_link_is_refused_by_the_gate() {
    let fx = Fx::new("p-dangling");
    if !can_link(&fx.s) {
        return;
    }
    link_file(&fx.s.path("nowhere"), &fx.at("dead")).unwrap();
    assert_eq!(
        fx.refusal(false, rename("dead", "alive")),
        "path escapes the workspace: dead"
    );
    assert_eq!(
        fx.refusal(false, folder(".", "dead")),
        "path escapes the workspace: dead"
    );
}

#[test]
fn a_drive_root_plans_lexically_and_folds_the_bin_s_case() {
    let h = drive_home(false);
    let space = Space::open(&h, "C:/Work/songs");
    let disk = Drive::new(
        &[
            "C:/Work/songs/a.txt",
            "C:/Work/songs/_recycle_bin/old",
            "C:/Work/songs/_RECYCLE_BIN/old",
        ],
        &[
            "C:/Work/songs",
            "C:/Work/songs/sub",
            "C:/Work/songs/_recycle_bin",
            "C:/Work/songs/_Recycle_Bin",
        ],
    );
    let p = |op: Op<'_>| plan(&space, &op, &disk);
    assert_eq!(
        p(folder("sub", "new")),
        Ok(Outcome::Folder {
            abs: "C:/Work/songs/sub/new".into()
        })
    );
    assert_eq!(
        p(folder("_RECYCLE_BIN", "x")),
        Err(EntryError::Refused(
            "can't create a folder in the recycle bin".into()
        ))
    );
    assert_eq!(
        p(folder(".", "sub")),
        Err(EntryError::Exists { rel: "sub".into() })
    );
    assert_eq!(
        p(rename("a.txt", "b.txt")),
        Ok(Outcome::Rename {
            from: "C:/Work/songs/a.txt".into(),
            to: "C:/Work/songs/b.txt".into(),
            refs: None
        })
    );
    assert_eq!(
        p(mv("a.txt", "_Recycle_Bin")),
        Err(EntryError::Refused(
            "can't move something into the recycle bin — delete it instead".into()
        ))
    );
    assert_eq!(
        p(delete("a.txt")),
        Ok(Outcome::Recycle {
            from: "C:/Work/songs/a.txt".into(),
            to: "C:/Work/songs/_recycle_bin/a.txt".into()
        })
    );
    assert_eq!(
        p(delete("_RECYCLE_BIN/old")),
        Err(EntryError::Refused(
            "'_RECYCLE_BIN/old' is already in the recycle bin".into()
        ))
    );
    assert_eq!(
        p(upload("sub", "x/y.bin")),
        Ok(Outcome::Upload {
            abs: "C:/Work/songs/sub/x/y.bin".into()
        })
    );
    assert_eq!(to_rel(&space, "C:\\Work\\songs\\sub"), "sub");
    // A drive path on a POSIX-rooted rel escapes, and a drive's own root is not an upload's rel.
    assert!(matches!(p(delete("C:/other")), Err(EntryError::Path(_))));
}

#[test]
fn a_move_onto_a_name_taken_by_a_dangling_link_finds_nothing_there() {
    let fx = Fx::new("p-dangling-dest");
    if !can_link(&fx.s) {
        return;
    }
    fx.s.file("studio/workspace/songs/d", "d");
    fx.s.dir("studio/workspace/songs/holder");
    link_file(&fx.s.path("nowhere"), &fx.at("holder/d")).unwrap();
    // `existsSync` follows the link, finds nothing, and the rename replaces the link (1222).
    assert_eq!(
        fx.user(mv("d", "holder")),
        Ok(Outcome::Move {
            from: fx.at("d"),
            to: fx.at("holder/d")
        })
    );
}

#[test]
fn a_home_spelt_with_backslashes_still_refuses_the_libraries_folder_before_any_escape() {
    let h = k7_file_service::libraries::Home {
        studio: "C:\\Work\\studio".into(),
        libraries: "C:\\Work\\libs".into(),
        admin: false,
    };
    let space = Space::open(&h, "C:/Work/studio/workspace/songs");
    let disk = Drive::new(&[], &["C:/Work/libs", "C:/Work/studio/workspace/songs"]);
    let said = "can't rename '../../../libs': the libraries are listed by the admins, not moved from a workspace";
    assert_eq!(
        plan(&space, &rename("_libraries", "libs"), &disk),
        Err(EntryError::Refused(said.into()))
    );
    assert_eq!(
        plan(&space, &mv("_libraries", "."), &disk),
        Err(EntryError::Refused(said.replace("rename", "move")))
    );
}
