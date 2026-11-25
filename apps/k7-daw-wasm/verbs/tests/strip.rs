// strip.rs — an event's own strip on real fixtures: fades, gain, window, loop and the two flags, as the engine then plays them.
// A reader would get wrong the units: a fade is a DURATION in the meter, a window seconds of the file, loop points the material's unit.

mod support;

use song::clips::ClipDoc;
use support::Project;

fn at(p: &mut Project, lane: &str, beat: f64) -> ClipDoc {
    p.track(lane)
        .clips
        .into_iter()
        .find(|c| (c.start - beat).abs() < 1e-6)
        .unwrap_or_else(|| panic!("no event at {beat}"))
}

#[test]
fn fades_are_set_and_removed() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("event fadein drums 1:1:1 0:0:2");
    assert_eq!(ran.answer.log, ["drums: kick at 1:1:1 fades in over 0:0:2"]);
    p.ok("event fadein drums 1:1:1 0:0:2");
    p.ok("event fadeout drums 1:1:1 0:0:1");
    let kick = at(&mut p, "drums", 0.0);
    assert!(
        kick.fade_in > 0.0 && kick.fade_out > 0.0,
        "{} {}",
        kick.fade_in,
        kick.fade_out
    );
    p.ok("event fadein drums 1:1:1 0");
    assert_eq!(at(&mut p, "drums", 0.0).fade_in, 0.0);
    p.refused("event fadein drums 1:1:1 x", "<length> expects");
    p.refused("event fadein drums 1:1:1", "usage: event fadein");
}

#[test]
fn a_gain_trims_one_audio_event_and_unity_clears_it() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("event gain drums 1:3:1 -2.25");
    assert_eq!(ran.answer.log, ["drums: kick at 1:3:1 trimmed -2.2 dB"]);
    p.ok("event gain drums 1:3:1 -2.25");
    assert_eq!(at(&mut p, "drums", 2.0).gain_db, Some(-2.2));
    p.ok("event gain drums 1:3:1 0");
    assert_eq!(at(&mut p, "drums", 2.0).gain_db, None);
    p.refused("event gain drums 1:3:1 9", "is not a gain from -6 to +6 dB");
    let mut seq = Project::open("edit-undo");
    seq.refused("event gain melody 1:1:1 2", "is MIDI");
}

#[test]
fn mute_and_warp_flags_follow_the_named_state() {
    let mut p = Project::open("clip-lane");
    assert_eq!(at(&mut p, "drums", 3.0).disabled, Some(true));
    p.round_trip("event status mute drums 1:4:1");
    p.ok("event status mute drums 1:4:1");
    assert_ne!(at(&mut p, "drums", 3.0).disabled, Some(true));
    p.ok("event status mute drums 1:1:1 on");
    assert_eq!(at(&mut p, "drums", 0.0).disabled, Some(true));
    p.ok("event status warp drums 1:1:1 off");
    // The doc says `warpOff` only for a clip with a warp map; the file says it for every clip.
    assert!(
        p.text("nodes/drums.k7x")
            .contains("start(\"1:1:1\").disabled(true).warp(false)")
    );
    p.ok("event status warp drums 1:1:1 on");
    assert!(!p.text("nodes/drums.k7x").contains("warp(false)"));
    p.refused("event status mute drums 1:1:1 sideways", "is not on, off or toggle");
}

#[test]
fn a_window_is_seconds_of_the_file() {
    let mut p = Project::open("clip-lane");
    p.round_trip("event window drums 1:1:1 0.01 0.08");
    p.ok("event window drums 1:1:1 0.01 0.08");
    let kick = at(&mut p, "drums", 0.0);
    assert_eq!((kick.source_from, kick.source_to), (Some(0.01), Some(0.08)));
    p.refused("event window drums 1:1:1 0.08 0.01", "has to run forwards");
    p.refused("event window drums 1:1:1 -1 2", "is not a window in seconds");
}

#[test]
fn a_loop_repeats_the_window_and_its_points_move_it() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("event loop drums 1:3:1 on");
    assert_eq!(ran.answer.log, ["drums: kick at 1:3:1 loops, 2 passes"]);
    p.ok("event loop drums 1:3:1 on");
    let kick = at(&mut p, "drums", 2.0);
    assert!(kick.loop_end > kick.loop_start, "the engine loops it");
    let again = p.ok("event loop drums 1:3:1 on");
    assert!(
        again.answer.effects.is_empty() && again.before == again.after,
        "already on writes nothing"
    );
    p.ok("event loop drums 1:3:1");
    assert!(at(&mut p, "drums", 2.0).loop_end <= at(&mut p, "drums", 2.0).loop_start);
    p.ok("event loop points drums 1:1:1 0.02 0.05");
    let kick = at(&mut p, "drums", 0.0);
    assert_eq!((kick.source_from, kick.source_to), (Some(0.02), Some(0.05)));
    p.refused("event loop points drums 1:1:1 0.05 0.02", "are not loop points");
}

#[test]
fn a_sequence_loops_in_beats() {
    let mut p = Project::open("edit-undo");
    p.round_trip("event loop melody 1:1:1 on");
    p.ok("event loop melody 1:1:1 on");
    let ran = p.ok("event loop points melody 1:1:1 1 3");
    assert_eq!(ran.answer.log, ["melody: melody at 1:1:1 loops 1b → 3b, 4 passes"]);
    let m = at(&mut p, "melody", 0.0);
    assert!(m.loop_end > m.loop_start);
}
