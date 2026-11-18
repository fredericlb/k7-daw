//! `plan_rename` and `plan_recycle` (2041 to 2043, 2094): the guards in the TypeScript's order and their sentences,
//! `was_active` by ancestry for a rename and by equality for a recycle, the bin's names, and the disk's part.

use crate::support;

use std::cell::Cell;
use std::io;
use std::time::Duration;

use k7_file_service::confine::Disk;
use k7_file_service::entries::apply::Renamer;
use k7_file_service::projects::ProjError;
use k7_file_service::projects::life::{
    MovePlan, apply_recycle, apply_rename, kept_answer, plan_recycle, plan_rename,
};
use k7_file_service::space::Space;
use support::entries::{Drive, drive_home};
use support::projects::songs;
use support::ws::Acct;

fn said<T: std::fmt::Debug>(r: Result<T, ProjError>) -> String {
    match r {
        Err(e) => e.to_string(),
        Ok(v) => panic!("not refused: {v:?}"),
    }
}

fn rename(a: &Acct, root: &str, rel: &str, name: &str) -> Result<Option<MovePlan>, ProjError> {
    let h = a.home();
    plan_rename(&Space::open(&h, root), false, rel, name, &Disk)
}

fn recycle(a: &Acct, root: &str, rel: &str) -> Result<MovePlan, ProjError> {
    let h = a.home();
    plan_recycle(&Space::open(&h, root), false, rel, &Disk)
}

#[test]
fn a_rename_answers_where_the_folder_lands() {
    let (a, root) = songs("pl-rename");
    let plan = rename(&a, &root, "a.k7", "b.k7").unwrap().unwrap();
    assert_eq!(
        (plan.abs.as_str(), plan.dest.as_str()),
        (
            format!("{root}/a.k7").as_str(),
            format!("{root}/b.k7").as_str()
        )
    );
    assert_eq!(plan.answer, r#"{"path":"b.k7"}"#);
    assert!(!plan.was_active);
}

#[test]
fn a_rename_to_the_name_it_has_is_none_and_the_answer_is_its_path() {
    let (a, root) = songs("pl-same");
    assert_eq!(rename(&a, &root, "a.k7", "a.k7"), Ok(None));
    assert_eq!(rename(&a, &root, "./a.k7/", " a.k7 "), Ok(None));
    let h = a.home();
    assert_eq!(
        kept_answer(&Space::open(&h, &root), "./a.k7/", &Disk).unwrap(),
        r#"{"path":"a.k7"}"#
    );
}

#[test]
fn a_rename_s_guards_run_in_order_with_their_sentences() {
    let (a, root) = songs("pl-guards");
    a.s.dir("studio/workspace/songs/notes");
    a.s.dir("studio/workspace/songs/_recycle_bin/x.k7");
    a.project("songs", "taken.k7", "");
    assert_eq!(
        said(rename(&a, &root, ".", "b.k7")),
        "can't rename the workspace root"
    );
    assert_eq!(
        said(rename(&a, &root, "_recycle_bin/x.k7", "b.k7")),
        "'_recycle_bin/x.k7' is already in the recycle bin"
    );
    assert_eq!(
        said(rename(&a, &root, "notes", "b.k7")),
        "not a project (the folder must end in .k7): notes"
    );
    assert_eq!(
        said(rename(&a, &root, "../a.k7", "b.k7")),
        "path escapes the workspace: ../a.k7"
    );
    assert_eq!(
        said(rename(&a, &root, "a.k7", "b.v1")),
        "a project keeps the suffix that says which app opens it — 'b.v1' should end in '.k7'"
    );
    assert_eq!(
        said(rename(&a, &root, "a.k7", ".k7")),
        "a project keeps the suffix that says which app opens it — '.k7' should end in '.k7'"
    );
    assert_eq!(
        rename(&a, &root, "a.k7", "taken.k7"),
        Err(ProjError::Exists("taken.k7".into()))
    );
}

#[test]
fn a_rename_s_name_is_cleaned_as_the_shim_cleans_it() {
    let (a, root) = songs("pl-clean");
    for (name, dest) in [
        ("x/y.k7", "y.k7"),
        ("x\\y.k7", "y.k7"),
        ("  z.k7  ", "z.k7"),
        ("a\\b.k7", "b.k7"),
        ("dir/z.k7/", "z.k7"),
    ] {
        let plan = rename(&a, &root, "a.k7", name).unwrap().unwrap();
        assert_eq!(plan.dest, format!("{root}/{dest}"), "{name:?}");
    }
}

#[test]
fn a_library_s_project_is_refused_to_an_account_that_may_not_edit_it() {
    let (a, root) = songs("pl-lib");
    a.s.dir("libraries/drums/demo.k7");
    let sentence =
        "can't rename '_libraries/drums/demo.k7': only the origin's admins edit a library";
    assert_eq!(
        said(rename(&a, &root, "_libraries/drums/demo.k7", "x.k7")),
        sentence
    );
    let del = "can't delete '_libraries/drums/demo.k7': only the origin's admins edit a library";
    assert_eq!(said(recycle(&a, &root, "_libraries/drums/demo.k7")), del);
}

#[test]
fn an_admin_s_recycle_of_a_library_s_project_goes_to_the_library_s_own_bin() {
    // W7: the library's bin, never the workspace's, so no account's library loses a project to another's bin.
    let (a, root) = songs("pl-libbin");
    a.s.dir("libraries/drums/demo.k7");
    a.s.dir("libraries/drums/_recycle_bin/demo.k7");
    let home = a.s.home(true);
    let plan = plan_recycle(
        &Space::open(&home, &root),
        true,
        "_libraries/drums/demo.k7",
        &Disk,
    )
    .unwrap();
    let libs = a.s.path("libraries");
    assert_eq!(plan.dest, format!("{libs}/drums/_recycle_bin/demo.k7-2"));
    assert_eq!(
        plan.answer,
        r#"{"recycled":"_libraries/drums/_recycle_bin/demo.k7-2"}"#
    );
}

#[test]
fn the_open_project_and_its_parent_both_move_a_rename_s_marker() {
    let (a, root) = songs("pl-active");
    a.project("songs", "outer.k7/inner.k7", "");
    a.marker("songs", "a.k7\n");
    assert!(
        rename(&a, &root, "a.k7", "b.k7")
            .unwrap()
            .unwrap()
            .was_active
    );
    assert!(
        !rename(&a, &root, "outer.k7", "o2.k7")
            .unwrap()
            .unwrap()
            .was_active
    );
    a.marker("songs", "outer.k7/inner.k7\n");
    assert!(
        rename(&a, &root, "outer.k7", "o2.k7")
            .unwrap()
            .unwrap()
            .was_active
    );
}

#[test]
fn a_rename_of_a_folder_above_the_open_project_carries_the_inner_one_with_it() {
    // F3: the marker names `o2.k7/inner.k7`, not the renamed folder.
    let (a, root) = songs("pl-carry");
    a.project("songs", "outer.k7/inner.k7", "");
    a.marker("songs", "outer.k7/inner.k7\n");
    let plan = rename(&a, &root, "outer.k7", "o2.k7").unwrap().unwrap();
    assert_eq!(plan.active_to, Some(format!("{root}/o2.k7/inner.k7")));
    a.marker("songs", "outer.k7\n");
    let plan = rename(&a, &root, "outer.k7", "o2.k7").unwrap().unwrap();
    assert_eq!(plan.active_to, Some(format!("{root}/o2.k7")));
    a.marker("songs", "a.k7\n");
    let plan = rename(&a, &root, "outer.k7", "o2.k7").unwrap().unwrap();
    assert_eq!(plan.active_to, None);
}

#[cfg(unix)]
#[cfg(unix)]
#[test]
fn a_marker_that_cannot_be_read_declines_both_plans_instead_of_reading_as_none() {
    // F2: `was_active` decides a write, so a transient EACCES must not read as "no project open".
    use k7_file_service::space::Declined;
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let (a, root) = songs("pl-eacces");
    a.marker("songs", "a.k7\n");
    let marker = a.s.path("studio/workspace/.active-projects/songs");
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o000)).unwrap();
    let renamed = rename(&a, &root, "a.k7", "b.k7");
    let recycled = recycle(&a, &root, "a.k7");
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o644)).unwrap();
    let declined = ProjError::Declined(Declined::WouldThrow);
    assert_eq!(renamed, Err(declined.clone()));
    assert_eq!(recycled, Err(declined));
}

#[test]
fn a_recycle_names_the_bin_s_first_free_name_and_says_where() {
    let (a, root) = songs("pl-recycle");
    let plan = recycle(&a, &root, "a.k7").unwrap();
    assert_eq!(plan.dest, format!("{root}/_recycle_bin/a.k7"));
    assert_eq!(plan.answer, r#"{"recycled":"_recycle_bin/a.k7"}"#);
    a.s.dir("studio/workspace/songs/_recycle_bin/a.k7");
    assert_eq!(
        recycle(&a, &root, "a.k7").unwrap().answer,
        r#"{"recycled":"_recycle_bin/a.k7-2"}"#
    );
    a.s.dir("studio/workspace/songs/_recycle_bin/a.k7-2");
    assert_eq!(
        recycle(&a, &root, "a.k7").unwrap().answer,
        r#"{"recycled":"_recycle_bin/a.k7-3"}"#
    );
}

#[test]
fn a_recycle_s_guards_run_in_order_with_their_sentences() {
    let (a, root) = songs("pl-rguards");
    a.s.dir("studio/workspace/songs/notes");
    a.s.dir("studio/workspace/songs/_recycle_bin/x.k7");
    assert_eq!(
        said(recycle(&a, &root, ".")),
        "can't delete the workspace root"
    );
    assert_eq!(
        said(recycle(&a, &root, "_recycle_bin/x.k7")),
        "already in the recycle bin: _recycle_bin/x.k7"
    );
    assert_eq!(
        said(recycle(&a, &root, "notes")),
        "not a project (the folder must end in .k7): notes"
    );
    assert_eq!(
        said(recycle(&a, &root, "../a.k7")),
        "path escapes the workspace: ../a.k7"
    );
}

#[test]
fn a_recycle_of_an_outer_project_does_not_take_the_open_inner_one_with_it() {
    // 2042: `===`, not ancestry; the marker keeps naming the inner project, now in the bin.
    let (a, root) = songs("pl-outer");
    a.project("songs", "outer.k7/inner.k7", "");
    a.marker("songs", "outer.k7/inner.k7\n");
    assert!(!recycle(&a, &root, "outer.k7").unwrap().was_active);
    a.marker("songs", "a.k7\n");
    assert!(recycle(&a, &root, "a.k7").unwrap().was_active);
    a.marker("songs", "outer.k7\n");
    assert!(recycle(&a, &root, "outer.k7").unwrap().was_active);
}

#[test]
fn a_drive_home_plans_both_acts_through_a_fake_disk() {
    for root in [
        "C:/Work/studio/workspace/songs",
        "c:/Work/studio/workspace/songs",
    ] {
        let dirs = [
            root.to_string(),
            format!("{root}/a.k7"),
            format!("{root}/_recycle_bin/a.k7"),
        ];
        let dirs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        let disk = Drive::new(&[], &dirs);
        let h = drive_home(false);
        let sp = Space::open(&h, root);
        let base = sp.root().to_string();
        let plan = plan_rename(&sp, false, "a.k7", "b.k7", &disk)
            .unwrap()
            .unwrap();
        assert_eq!(
            (plan.abs, plan.dest),
            (format!("{base}/a.k7"), format!("{base}/b.k7"))
        );
        assert_eq!(
            plan_rename(&sp, false, "a.k7", "A.k7", &disk)
                .unwrap()
                .unwrap()
                .answer,
            r#"{"path":"A.k7"}"#
        );
        let bin = plan_recycle(&sp, false, "a.k7", &disk).unwrap();
        assert_eq!(bin.dest, format!("{base}/_recycle_bin/a.k7-2"));
        assert!(!bin.dest.contains(r"\\?\"));
    }
}

struct Flaky {
    fail: Cell<u32>,
    calls: Cell<u32>,
    kind: io::ErrorKind,
}

impl Renamer for Flaky {
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        self.calls.set(self.calls.get() + 1);
        if self.fail.get() > 0 {
            self.fail.set(self.fail.get() - 1);
            return Err(io::Error::from(self.kind));
        }
        std::fs::rename(from, to)
    }
}

#[test]
fn the_disk_s_part_moves_the_folder_and_makes_the_bin() {
    let (a, root) = songs("pl-apply");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let plan = plan_recycle(&sp, false, "a.k7", &Disk).unwrap();
    apply_recycle(
        &sp,
        &plan,
        &k7_file_service::entries::apply::Disk,
        3,
        Duration::ZERO,
    )
    .unwrap();
    assert!(!std::path::Path::new(&plan.abs).exists());
    assert!(std::path::Path::new(&format!("{root}/_recycle_bin/a.k7/project.k7x")).exists());
    a.project("songs", "c.k7", "");
    let mv = plan_rename(&sp, false, "c.k7", "d.k7", &Disk)
        .unwrap()
        .unwrap();
    apply_rename(
        &mv,
        &k7_file_service::entries::apply::Disk,
        3,
        Duration::ZERO,
    )
    .unwrap();
    assert!(std::path::Path::new(&format!("{root}/d.k7/project.k7x")).exists());
}

#[test]
fn a_rename_waits_out_a_handle_that_lets_go() {
    let (a, root) = songs("pl-retry");
    let plan = rename(&a, &root, "a.k7", "b.k7").unwrap().unwrap();
    let held = Flaky {
        fail: Cell::new(2),
        calls: Cell::new(0),
        kind: io::ErrorKind::PermissionDenied,
    };
    apply_rename(&plan, &held, 5, Duration::ZERO).unwrap();
    assert_eq!(held.calls.get(), 3);
    assert!(std::path::Path::new(&plan.dest).exists());
}

#[test]
fn a_recycle_under_a_held_handle_answers_its_code() {
    let (a, root) = songs("pl-held");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let plan = plan_recycle(&sp, false, "a.k7", &Disk).unwrap();
    let held = Flaky {
        fail: Cell::new(u32::MAX),
        calls: Cell::new(0),
        kind: io::ErrorKind::ResourceBusy,
    };
    let err = apply_recycle(&sp, &plan, &held, 4, Duration::ZERO).unwrap_err();
    assert!(
        matches!(
            &err,
            ProjError::Io {
                code: "EBUSY",
                syscall: "rename",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(held.calls.get(), 5, "the first try and four retries");
    assert!(std::path::Path::new(&plan.abs).exists(), "nothing moved");
}

#[test]
fn a_move_that_is_not_transient_is_answered_at_once() {
    let (a, root) = songs("pl-once");
    let plan = rename(&a, &root, "a.k7", "b.k7").unwrap().unwrap();
    let gone = Flaky {
        fail: Cell::new(u32::MAX),
        calls: Cell::new(0),
        kind: io::ErrorKind::NotFound,
    };
    let err = apply_rename(&plan, &gone, 25, Duration::ZERO).unwrap_err();
    assert!(matches!(err, ProjError::Io { code: "ENOENT", .. }));
    assert_eq!(gone.calls.get(), 1);
}
