// clips_negative.rs — the placements' negative corpus: each case a project laid out as a module case, with the file sets it must give.
// A reader would get wrong that this needs no TypeScript: stages_clips.rs adds the TypeScript's answers to the cases that can have one.

use crate::clips_support;
use crate::support;

use clips_support::{Counts, check, clip_cases, lay, run};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use support::{Tmp, case_roots};

#[test]
fn every_case_gives_the_files_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("clips-negative");
    let mut seen = Counts::default();
    let mut failures = Vec::new();
    for case in clip_cases() {
        let base = t.0.join(&case.name);
        lay(&base, &case);
        let roots = case_roots(&base, None);
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = run(&rec, &mut Cache::new(), &roots).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        failures.extend(check(&case, &ran));
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
    for (what, n) in [
        ("takes", seen.takes),
        ("comp pieces", seen.comp_pieces),
        ("crossfade marks", seen.crossfades),
        ("timeline docs", seen.timeline_docs),
        ("nested clips", seen.nested),
        ("seconds starts", seen.seconds_starts),
        ("fractional ids", seen.fractional_ids),
        ("loop ranges", seen.loop_ranges),
        ("event windows", seen.event_windows),
        ("clip windows", seen.clip_windows),
        ("gains", seen.gains),
        ("warps", seen.warps),
        ("warp_offs", seen.warp_offs),
        ("disabled", seen.disabled),
        ("pad effects", seen.pad_effects),
    ] {
        assert!(n >= 1, "the corpus placed no {what}");
    }
    assert!(clip_cases().len() >= 55 && clip_cases().iter().filter(|c| c.ts).count() >= 40);
}
