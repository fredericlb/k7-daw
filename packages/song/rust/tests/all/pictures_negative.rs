// pictures_negative.rs — the pictures' negative corpus: each case a project laid out as a module case, with the files and faults it must give.
// A reader would get wrong that this needs no TypeScript: stages_pictures.rs adds the TypeScript's answers to the cases that can have one.

use crate::pictures_support;
use crate::support;

use pictures_support::{Counts, check_case, lay_case, pic_cases, run};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use support::{Tmp, case_roots};

#[test]
fn every_case_gives_the_files_and_faults_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("pictures-negative");
    let mut seen = Counts::default();
    let mut failures = Vec::new();
    for case in pic_cases() {
        let base = t.0.join(&case.name);
        lay_case(&base, &case, support::placed);
        let roots = case_roots(&base, None);
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = run(&rec, &mut Cache::new(), &roots).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        failures.extend(check_case(&case, &ran));
        seen.add(&ran.counts);
        if case.mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            let again = run(&mem, &mut Cache::new(), &roots)
                .unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
            if again.json != ran.json || again.refused != ran.refused || again.warned != ran.warned {
                failures.push(format!("{}: the bundle read differently from the disk", case.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    eprintln!("negative corpus counts: {seen:?}");
    for (what, n) in [
        ("pieces", seen.pieces),
        ("re-entries", seen.reentered),
        ("shots cut twice", seen.multi_cut),
        ("crossfades", seen.crossfades),
        ("last-in-file crossfades", seen.cross_ties),
        ("open pieces", seen.open_pieces),
        ("trimmed pieces", seen.from_pieces),
        ("varispeed warps", seen.varispeed),
        ("elastique warps", seen.elastique),
        ("silent shots covering", seen.silent_cover),
        ("freeze shots covering", seen.freeze_cover),
        ("reverse shots covering", seen.reverse_cover),
        ("lanes sounded through video_source", seen.video_lanes),
        ("lanes sounded by their own half", seen.own_lanes),
        ("wired lanes with no clips", seen.wired_empty),
        ("both faults", seen.fault_both),
        ("absent faults", seen.fault_absent),
        ("missing faults", seen.fault_missing),
        ("placed shots", seen.placed),
        ("placed shots on a nest", seen.nested),
        ("placed shots cut at the end", seen.cut),
        ("placed shots given a length", seen.given),
        ("placed shots merged first", seen.merged),
        ("placed shots that sounded", seen.sounded),
    ] {
        assert!(n >= 1, "the corpus held no {what}");
    }
    let cases = pic_cases();
    assert!(cases.len() >= 60 && cases.iter().filter(|c| c.ts).count() >= 55);
}
