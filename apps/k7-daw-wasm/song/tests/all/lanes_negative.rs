// lanes_negative.rs — the negative corpus of 2e: each case a project laid out as a module case, with the sets and pointers it must give.
// A reader would get wrong that this needs no TypeScript: stages_lanes.rs adds the TypeScript's answers to the cases that can have one.

use crate::lanes_support;
use crate::support;

use lanes_support::{Counts, check, lane_cases, lay, run};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use support::{Tmp, case_roots};

#[test]
fn every_case_gives_what_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("lanes-negative");
    let mut seen = Counts::default();
    let mut failures = Vec::new();
    for case in lane_cases() {
        let base = t.0.join(&case.name);
        lay(&base, &case);
        let roots = case_roots(&base, None);
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let ran = run(&rec, &mut Cache::new(), &roots, &case.rig).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        failures.extend(check(&case, &ran));
        seen.add(&ran.counts);
        if case.mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            let again = run(&mem, &mut Cache::new(), &roots, &case.rig)
                .unwrap_or_else(|e| panic!("{}: the bundle was not enough: {e}", case.name));
            if again.json != ran.json || again.stopped != ran.stopped || again.warned != ran.warned {
                failures.push(format!("{}: the bundle read differently from the disk", case.name));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    for (what, n) in [
        ("tap chains", seen.tap),
        ("console halves", seen.halves),
        ("live inputs with channels", seen.live_channels),
        ("cue pairs applied", seen.cue_applied),
    ] {
        assert!(n >= 1, "the corpus has no {what}");
    }
    for kind in [
        "extends_broken",
        "extends_unknown",
        "timeline_cycle",
        "timeline_fixed",
        "clip_broken",
        "sequence_broken",
        "ref_missing",
        "audio_outside",
        "audio_missing",
        "sequence_gain",
        "loop_no_to",
        "timeline_broken",
        "strudel_missing",
        "group_source",
        "too_few",
        "too_many",
        "port_type",
        "tap_unrouted",
        "console_absent",
        "more",
    ] {
        if !seen.kinds.contains(kind) {
            failures.push(format!("no case gives a {kind} fault"));
        }
    }
    for source in [
        "clip",
        "sequence",
        "lane_port",
        "lane_audio",
        "lane_midi",
        "lane_mixed",
        "picture",
        "picture_audio",
        "unknown",
        "group",
    ] {
        if !seen.sources.contains(source) {
            failures.push(format!("no case wires a {source} source"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let cases = lane_cases();
    assert!(cases.len() >= 70 && cases.iter().filter(|c| c.ts).count() >= 55);
}
