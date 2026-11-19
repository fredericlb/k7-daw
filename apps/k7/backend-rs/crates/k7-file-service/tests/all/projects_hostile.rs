//! The hostile inputs of the projects' plans that no single table holds: a bin of a thousand, a rename that is one
//! call whatever the folder holds, a workspace of many folders. Bounds are counts, never a wall time.

use crate::support;

use std::cell::Cell;
use std::io;
use std::time::Duration;

use k7_file_service::entries::apply::Renamer;
use k7_file_service::projects::life::{apply_rename, plan_recycle, plan_rename};
use k7_file_service::projects::open::default_project;
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use support::projects::{Counting, songs};

#[test]
fn a_thousand_recycles_of_one_name_find_the_bins_one_thousandth() {
    let (a, root) = songs("ph-bin");
    a.s.dir("studio/workspace/songs/_recycle_bin/a.k7");
    for n in 2..=999 {
        a.s.dir(&format!("studio/workspace/songs/_recycle_bin/a.k7-{n}"));
    }
    let h = a.home();
    let disk = Counting::default();
    let plan = plan_recycle(&Space::open(&h, &root), "a.k7", &disk).unwrap();
    assert_eq!(plan.answer, r#"{"recycled":"_recycle_bin/a.k7-1000"}"#);
    // One probe per name tried, and the gate's own, never one per name already seen twice.
    let exists = disk.exists.load(std::sync::atomic::Ordering::Relaxed);
    assert!((1000..=1010).contains(&exists), "{exists} probes");
}

struct Once(Cell<u32>);

impl Renamer for Once {
    fn rename(&self, _from: &str, _to: &str) -> io::Result<()> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}

#[test]
fn a_rename_is_one_call_whatever_the_folder_holds() {
    let (a, root) = songs("ph-one");
    for n in 0..300 {
        a.s.file(&format!("studio/workspace/songs/a.k7/f{n}/clip.wav"), "x");
    }
    let h = a.home();
    let disk = Counting::default();
    let plan = plan_rename(&Space::open(&h, &root), "a.k7", "b.k7", &disk)
        .unwrap()
        .unwrap();
    assert!(disk.total() < 40, "{} probes", disk.total());
    let once = Once(Cell::new(0));
    apply_rename(&plan, &once, 25, Duration::ZERO).unwrap();
    assert_eq!(once.0.get(), 1);
}

#[test]
fn a_workspace_of_many_folders_is_declined_by_its_count_not_walked_out() {
    let (a, root) = songs("ph-many");
    for n in 0..300 {
        a.s.dir(&format!("studio/workspace/songs/d{n}/e"));
    }
    let h = a.home();
    for max in [50, 200] {
        let limits = Limits {
            max_seen: max,
            ..Limits::default()
        };
        assert_eq!(
            default_project(&Space::open(&h, &root), &limits),
            Err(Declined::TooMany),
            "{max}"
        );
    }
    let wide = Limits {
        max_seen: 10_000,
        ..Limits::default()
    };
    assert_eq!(
        default_project(&Space::open(&h, &root), &wide),
        Ok(Some("a.k7".into()))
    );
}

#[test]
fn the_spelling_lookup_reads_the_folders_it_names_counted_and_bounded() {
    // F5: one lookup per segment, each reading its folder once; the reads grow with the folder (n against 4n) and stop at the bound.
    use std::sync::atomic::Ordering::Relaxed;
    let (a, root) = songs("ph-spell");
    let reads = |n: usize| {
        for i in 0..n {
            a.s.dir(&format!("studio/workspace/songs/crowd{n}/f{i}"));
        }
        a.s.dir(&format!("studio/workspace/songs/crowd{n}/p.k7"));
        let (h, disk) = (a.home(), Counting::default());
        let rel = format!("crowd{n}/p.k7");
        plan_rename(&Space::open(&h, &root), &rel, "q.k7", &disk).unwrap();
        (disk.spells.load(Relaxed), disk.read.load(Relaxed))
    };
    let ((spells, small), (spells_big, large)) = (reads(10), reads(40));
    assert_eq!(spells, 2);
    assert_eq!(spells_big, 2);
    assert!(
        small >= 10 && large >= 40 && large <= 4 * small + 8,
        "{small}, {large}"
    );
    let (h, disk) = (a.home(), Counting::default());
    disk.cap.store(20, Relaxed);
    let declined = plan_rename(&Space::open(&h, &root), "crowd40/p.k7", "q.k7", &disk);
    assert!(
        matches!(
            declined,
            Err(k7_file_service::projects::ProjError::Declined(
                Declined::TooMany
            ))
        ),
        "{declined:?}"
    );
}
