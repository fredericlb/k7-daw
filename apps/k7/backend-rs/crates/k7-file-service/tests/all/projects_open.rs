//! `defaultProject` and `plan_open` (2030, 2031, 2092): which project a bare address opens, the guards in the
//! TypeScript's order, the sentences whole, the `C:/` spelling through a fake disk, and the hostile paths.

use crate::support;

use k7_file_service::confine::Disk;
use k7_file_service::projects::ProjError;
use k7_file_service::projects::open::{OpenPlan, default_project, plan_open};
use k7_file_service::space::{Declined, MAX_REL, Space};
use k7_file_service::tree::Limits;
use support::entries::{Drive, drive_home};
use support::projects::{Counting, songs};
use support::ws::Acct;

fn refused(e: Result<OpenPlan, ProjError>) -> String {
    match e {
        Err(ProjError::Refused(s)) => s,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_workspace_that_is_a_project_opens_as_dot() {
    let a = Acct::new("po-rootproj");
    a.project("r.k7", "x.k7", "");
    a.s.file("studio/workspace/r.k7/project.k7x", "");
    let h = a.home();
    let sp = Space::open(&h, &a.path("r.k7", ""));
    assert_eq!(
        default_project(&sp, &Limits::default()),
        Ok(Some(".".into()))
    );
    let plan = plan_open(&sp, ".", &Disk).unwrap();
    assert_eq!((plan.rel.as_str(), plan.abs.as_str()), (".", sp.root()));
}

#[test]
fn the_marker_names_the_default_and_a_stale_one_falls_to_the_first_on_disk() {
    let (a, root) = songs("po-default");
    a.project("songs", "b.k7", "");
    let h = a.home();
    let limits = Limits::default();
    assert_eq!(
        default_project(&Space::open(&h, &root), &limits),
        Ok(Some("a.k7".into()))
    );
    a.marker("songs", "b.k7\n");
    assert_eq!(
        default_project(&Space::open(&h, &root), &limits),
        Ok(Some("b.k7".into()))
    );
    a.marker("songs", "gone.k7\n");
    assert_eq!(
        default_project(&Space::open(&h, &root), &limits),
        Ok(Some("a.k7".into()))
    );
    a.marker("songs", "_recycle_bin/b.k7\n");
    assert_eq!(
        default_project(&Space::open(&h, &root), &limits),
        Ok(Some("a.k7".into()))
    );
}

#[test]
fn a_workspace_with_no_project_has_no_default() {
    let a = Acct::new("po-empty");
    let root = a.ws("songs");
    a.s.dir("studio/workspace/songs/notes");
    let h = a.home();
    assert_eq!(
        default_project(&Space::open(&h, &root), &Limits::default()),
        Ok(None)
    );
}

#[test]
fn the_default_reads_a_marker_outside_the_root_that_the_active_reader_confines() {
    // 2030: `Space::active` says none for `../other.k7`, `defaultProject` names it and the open's gate refuses it.
    let (a, root) = songs("po-confine");
    a.s.file("studio/workspace/other.k7/project.k7x", "");
    a.marker("songs", "../other.k7\n");
    let h = a.home();
    let sp = Space::open(&h, &root);
    assert_eq!(sp.active(), Ok(None));
    assert_eq!(
        default_project(&sp, &Limits::default()),
        Ok(Some("../other.k7".into()))
    );
    let said = refused(plan_open(&sp, "../other.k7", &Disk));
    assert_eq!(said, "path escapes the workspace: ../other.k7");
}

#[test]
fn a_legacy_marker_or_a_walk_past_its_limit_declines_the_default() {
    let (a, root) = songs("po-declined");
    let h = a.home();
    a.s.file("studio/workspace/songs/.studio-active", "a.k7\n");
    assert_eq!(
        default_project(&Space::open(&h, &root), &Limits::default()),
        Err(Declined::LegacyMarker)
    );
    let b = Acct::new("po-many");
    let broot = b.ws("songs");
    for n in 0..40 {
        b.s.dir(&format!("studio/workspace/songs/d{n}"));
    }
    let tight = Limits {
        max_seen: 30,
        ..Limits::default()
    };
    let bh = b.home();
    assert_eq!(
        default_project(&Space::open(&bh, &broot), &tight),
        Err(Declined::TooMany)
    );
}

#[test]
fn the_guards_run_in_the_typescripts_order() {
    let (a, root) = songs("po-order");
    a.s.dir("studio/workspace/songs/notes");
    a.s.file("studio/workspace/songs/f.k7", "file");
    a.s.dir("studio/workspace/songs/_recycle_bin/x.k7");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let ok = plan_open(&sp, "a.k7", &Disk).unwrap();
    assert_eq!(
        ok,
        OpenPlan {
            abs: format!("{root}/a.k7"),
            rel: "a.k7".into()
        }
    );
    assert_eq!(
        refused(plan_open(&sp, "../a.k7", &Disk)),
        "path escapes the workspace: ../a.k7"
    );
    assert_eq!(
        refused(plan_open(&sp, "_recycle_bin/x.k7", &Disk)),
        "project is in the recycle bin: _recycle_bin/x.k7"
    );
    assert_eq!(
        refused(plan_open(&sp, "_recycle_bin/none.k7", &Disk)),
        "project is in the recycle bin: _recycle_bin/none.k7"
    );
    assert_eq!(
        refused(plan_open(&sp, "gone.k7", &Disk)),
        "no project 'gone.k7' in this workspace — it may have been renamed, removed, or never have been on this machine; go back to the projects page and open it from there"
    );
    for rel in ["notes", "f.k7", "."] {
        assert_eq!(
            refused(plan_open(&sp, rel, &Disk)),
            format!("not a project (the folder must end in .k7): {rel}")
        );
    }
}

#[test]
fn a_library_s_project_opens_by_its_virtual_path() {
    let (a, root) = songs("po-lib");
    a.s.dir("libraries/drums/demo.k7");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let plan = plan_open(&sp, "_libraries/drums/demo.k7", &Disk).unwrap();
    assert_eq!(plan.rel, "_libraries/drums/demo.k7");
    assert_eq!(plan.abs, a.s.path("libraries/drums/demo.k7"));
    assert_eq!(
        refused(plan_open(&sp, "_libraries/Bad Name/x.k7", &Disk)),
        "names no library: _libraries/Bad Name/x.k7"
    );
}

#[test]
fn a_name_of_the_folders_other_case_opens_the_folder_the_disk_holds() {
    // 2092: APFS and NTFS fold the case, ext4 does not; either way the plan names what the disk calls the folder.
    let (a, root) = songs("po-case");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let folds = std::fs::metadata(format!("{root}/A.K7")).is_ok();
    match plan_open(&sp, "A.k7", &Disk) {
        Ok(plan) => {
            assert!(folds || plan.rel == "A.k7");
            assert_eq!(plan.rel, if folds { "a.k7" } else { "A.k7" });
        }
        Err(e) => assert!(!folds, "{e:?}"),
    }
    if folds {
        let nested = plan_open(&sp, "A.k7/", &Disk).unwrap();
        assert_eq!(nested.rel, "a.k7");
        // The bin's name in another case is still the bin.
        a.s.dir("studio/workspace/songs/_recycle_bin/x.k7");
        let bin = plan_open(&sp, "_Recycle_Bin/x.k7", &Disk);
        assert_eq!(
            refused(bin),
            "project is in the recycle bin: _Recycle_Bin/x.k7"
        );
    }
}

#[test]
fn a_drive_home_opens_through_a_fake_disk_with_no_case_to_fold() {
    for root in [
        "C:/Work/studio/workspace/songs",
        "c:/Work/studio/workspace/songs",
    ] {
        let dirs = [
            root.to_string(),
            format!("{root}/a.k7"),
            format!("{root}/deep/b.k7"),
            format!("{root}/_recycle_bin/x.k7"),
        ];
        let dirs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        let file = format!("{root}/f.k7");
        let disk = Drive::new(&[&file], &dirs);
        let h = drive_home();
        let sp = Space::open(&h, root);
        let got = |rel: &str| plan_open(&sp, rel, &disk);
        let base = sp.root().to_string();
        assert_eq!(
            got("a.k7").unwrap(),
            OpenPlan {
                abs: format!("{base}/a.k7"),
                rel: "a.k7".into()
            }
        );
        assert_eq!(got("deep\\b.k7").unwrap().rel, "deep/b.k7");
        assert_eq!(
            got("./deep//b.k7").unwrap().abs,
            format!("{base}/deep/b.k7")
        );
        assert!(refused(got("A.k7")).starts_with("no project 'A.k7'"));
        assert_eq!(
            refused(got("_recycle_bin/x.k7")),
            "project is in the recycle bin: _recycle_bin/x.k7"
        );
        assert_eq!(
            refused(got("f.k7")),
            "not a project (the folder must end in .k7): f.k7"
        );
        assert_eq!(
            refused(got("../x.k7")),
            "path escapes the workspace: ../x.k7"
        );
        assert!(!got("a.k7").unwrap().abs.contains(r"\\?\"));
    }
}

#[test]
fn three_hundred_folders_above_a_project_cost_a_probe_each_and_no_more() {
    let (a, root) = songs("po-deep");
    let cost = |n: usize| {
        let rel = format!("t{n}/{}p.k7", "x/".repeat(n));
        a.s.dir(&format!("studio/workspace/songs/{rel}"));
        let h = a.home();
        let disk = Counting::default();
        let plan = plan_open(&Space::open(&h, &root), &rel, &disk).unwrap();
        assert_eq!(plan.rel, rel);
        disk.total()
    };
    let (small, large) = (cost(75), cost(300));
    assert!(
        large <= 4 * small + 8,
        "{small} probes for 75, {large} for 300"
    );
}

#[test]
fn a_name_of_sixteen_kibibytes_and_a_rel_past_the_limit_are_answered_without_a_hang() {
    let (a, root) = songs("po-long");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let long = format!("{}.k7", "n".repeat(8 * 1024));
    assert!(refused(plan_open(&sp, &long, &Disk)).starts_with("no project"));
    let sixteen = format!("{}.k7", "n".repeat(16 * 1024));
    assert_eq!(
        plan_open(&sp, &sixteen, &Disk),
        Err(ProjError::Declined(Declined::LongPath))
    );
    let past = "x/".repeat(MAX_REL);
    assert_eq!(
        plan_open(&sp, &past, &Disk),
        Err(ProjError::Declined(Declined::LongPath))
    );
}

#[test]
fn a_workspace_reached_through_a_link_keeps_its_own_spelling_never_the_canonical_one() {
    // The marker's rel and `activate`'s path are the lexical ones: `/var/...` stays `/var/...` where the disk says `/private/var/...`.
    let (a, real) = songs("po-spelled");
    let alias = a.s.path("alias");
    if support::link(&a.s.path("studio/workspace"), &alias).is_err() {
        return;
    }
    let root = format!("{alias}/songs");
    let h = a.home();
    let sp = Space::open(&h, &root);
    let plan = plan_open(&sp, "a.k7", &Disk).unwrap();
    assert_eq!(plan.abs, format!("{root}/a.k7"));
    assert_eq!(plan.rel, "a.k7");
    assert_ne!(plan.abs, format!("{real}/a.k7"));
}
