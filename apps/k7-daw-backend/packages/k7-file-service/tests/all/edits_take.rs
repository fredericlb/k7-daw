//! A finished take filed on the disk (3j M2): onto its lane, or into the event it was armed on; whole or not at all.

use crate::support;

use k7_file_service::edits::take::{Meter, file_take};
use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::wire::Take;
use support::Scratch;
use support::changes::tree;

const METER: Meter = Meter {
    tempo: 120.0,
    num: 4.0,
    den: 4.0,
};
const STAMP: &str = "261112T090503";
const LANE: &str = "extends(\"@meta/track\")\nevents([\n  ref(\"kick\").start(\"1:1:1\"),\n])\n";

fn seeded(name: &str) -> (Scratch, String, Take) {
    let s = Scratch::new(name);
    let dir = s.dir("p.k7");
    s.file("p.k7/project.k7x", "project({ name: \"p\", tempo: 120 })\n");
    s.file("p.k7/nodes/drums.k7x", LANE);
    s.file("p.k7/clips/kick.k7x", "source(\"kick.wav\")\n");
    let wav = s.file("p.k7/clips/tmp1.wav", "RIFF");
    let take = Take {
        track: "drums".into(),
        file: wav,
        start: 0.0,
        length: 4.0,
    };
    (s, dir, take)
}

// A take with no arm is declared as a clip, its file renamed, and placed on its lane.
#[test]
fn a_take_with_no_arm_is_placed_on_its_lane() {
    let (_s, dir, take) = seeded("t-lane");
    let (id, landed) = file_take(&dir, &take, None, METER, STAMP).unwrap();
    assert_eq!(id, "drums_1.1_261112T090503");
    assert!(landed.structural);
    let t = tree(&dir);
    assert!(t.contains_key("clips/drums_1.1_261112T090503.k7x"));
    assert!(t.contains_key("clips/drums_1.1_261112T090503.wav"));
    assert!(!t.contains_key("clips/tmp1.wav"));
    assert!(
        t["nodes/drums.k7x"].contains(&format!("ref(\"{id}\")")),
        "{}",
        t["nodes/drums.k7x"]
    );
}

// An engine on Windows reports the take with backslashes, and it is filed all the same.
#[cfg(windows)]
#[test]
fn a_take_spelled_with_backslashes_is_placed_on_its_lane() {
    let (_s, dir, mut take) = seeded("t-back");
    take.file = take.file.replace('/', "\\");
    let (id, _) = file_take(&dir, &take, None, METER, STAMP).unwrap();
    assert!(tree(&dir).contains_key(&format!("clips/{id}.wav")));
}

// A take whose track the arm names goes into that event as a take of it, and the lane gains no placement.
#[test]
fn an_armed_take_goes_into_its_event_not_onto_the_lane() {
    let (_s, dir, take) = seeded("t-armed");
    let arm = TakeArm {
        track: "drums".into(),
        event_beats: 0.0,
        from: 0.0,
        to: None,
    };
    let (id, _) = file_take(&dir, &take, Some(&arm), METER, STAMP).unwrap();
    let lane = &tree(&dir)["nodes/drums.k7x"];
    assert!(lane.contains(&id), "{lane}");
    assert!(lane.contains("takes"), "{lane}");
    assert_eq!(lane.matches("start(").count(), 2, "{lane}");
    // An arm on another track leaves this take to its lane.
    let (_s2, dir2, take2) = seeded("t-armed-other");
    let other = TakeArm {
        track: "bass".into(),
        ..arm
    };
    let (id2, _) = file_take(&dir2, &take2, Some(&other), METER, STAMP).unwrap();
    let lane2 = &tree(&dir2)["nodes/drums.k7x"];
    assert!(
        lane2.contains(&format!("ref(\"{id2}\")")) && !lane2.contains("takes"),
        "{lane2}"
    );
}

// A take whose file is gone, and a lane that has no file: the message, and nothing written, not even the rename.
#[test]
fn a_take_that_cannot_be_filed_writes_nothing() {
    let (_s, dir, mut take) = seeded("t-hostile");
    let before = tree(&dir);
    let ghost = Take {
        track: "ghost".into(),
        ..take.clone()
    };
    let err = file_take(&dir, &ghost, None, METER, STAMP).unwrap_err();
    assert!(err.contains("has no file"), "{err}");
    assert_eq!(tree(&dir), before);
    take.file = format!("{dir}/clips/gone.wav");
    let err = file_take(&dir, &take, None, METER, STAMP).unwrap_err();
    assert!(err.starts_with("recorded file is missing"), "{err}");
    assert_eq!(tree(&dir), before);
}

// O11: a clip's copy lands as a field of its file and touches nothing else; a clip file that will not read writes nothing.
#[test]
fn a_clip_copy_lands_as_one_field() {
    use k7_file_service::edits::prepare::{Copy, land_clip_copy};
    let (_s, dir, _) = seeded("t-copy");
    let landed = land_clip_copy(&dir, "kick", Copy::Optimized, "clips/kick.optimized.mp4").unwrap();
    assert_eq!(landed.touched.len(), 1);
    land_clip_copy(&dir, "kick", Copy::Proxy, "clips/kick.proxy.mp4").unwrap();
    let t = tree(&dir);
    let clip = &t["clips/kick.k7x"];
    assert!(
        clip.contains("optimized") && clip.contains("clips/kick.optimized.mp4"),
        "{clip}"
    );
    assert!(clip.contains("proxy") && clip.contains("source"), "{clip}");
    assert_eq!(t["nodes/drums.k7x"], LANE);
}

// A take outside the project's own clips folder is refused, and nothing is written or moved, whoever's file it names.
#[test]
fn a_take_outside_the_projects_clips_is_refused() {
    let (s, dir, mut take) = seeded("t-outside");
    let other = s.file("q.k7/project.k7x", "project({ name: \"q\", tempo: 120 })\n");
    let before = tree(&dir);
    let others = tree(&s.dir("q.k7"));
    take.file = other;
    let err = file_take(&dir, &take, None, METER, STAMP).unwrap_err();
    assert!(err.contains("not in the project's clips folder"), "{err}");
    assert_eq!(tree(&dir), before);
    assert_eq!(tree(&s.dir("q.k7")), others);
    // A folder below clips/ is outside as well.
    take.file = s.file("p.k7/clips/takes/deep.wav", "RIFF");
    assert!(file_take(&dir, &take, None, METER, STAMP).is_err());
}

// Two tracks whose ids sanitise alike, stopped in the same second: the second take takes a free name, the first stays.
#[test]
fn a_take_never_replaces_another_takes_file() {
    let (s, dir, take) = seeded("t-same");
    s.file("p.k7/nodes/lead synth.k7x", LANE);
    s.file("p.k7/nodes/lead-synth.k7x", LANE);
    let first = Take {
        track: "lead synth".into(),
        ..take.clone()
    };
    let (a, _) = file_take(&dir, &first, None, METER, STAMP).unwrap();
    let second = Take {
        track: "lead-synth".into(),
        file: s.file("p.k7/clips/tmp2.wav", "RIFF2"),
        ..take
    };
    let (b, _) = file_take(&dir, &second, None, METER, STAMP).unwrap();
    assert_ne!(a, b);
    let t = tree(&dir);
    assert_eq!(t[&format!("clips/{a}.wav")], "RIFF");
    assert_eq!(t[&format!("clips/{b}.wav")], "RIFF2");
}
