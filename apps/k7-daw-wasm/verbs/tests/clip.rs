// clip.rs — a clip's file and the verbs that need the engine, on real fixtures: warp points, onsets, crossfades, consolidate, and the refusals by host member.
// A reader would get wrong the crossfade's set-up: the fixture's hits are far apart, so B's window is opened first to give it material to overlap with.

mod support;

use k7x::edit::Value;
use support::Project;

fn markers(p: &Project, clip: &str) -> usize {
    match p
        .data(&format!("clips/{clip}.k7x"))
        .get("warp")
        .and_then(|w| w.get("markers"))
    {
        Some(Value::List(l)) => l.len(),
        _ => 0,
    }
}

#[test]
fn a_warp_point_is_placed_moved_and_removed_never_below_two() {
    let mut p = Project::open("clip-warp");
    let ran = p.round_trip("clip warp kick-warped 0.05 0.4");
    assert_eq!(ran.answer.log, ["kick-warped: 0.05s → beat 0.4 (3 points)"]);
    p.ok("clip warp kick-warped 0.05 0.4");
    assert_eq!(markers(&p, "kick-warped"), 3);
    let warp = p.track("drums").clips[0]
        .warp
        .clone()
        .expect("the engine is sent the map");
    assert_eq!(warp.markers.len(), 3);
    p.refused("clip warp kick-warped 0.05 2", "between its neighbours");
    p.round_trip("clip warp remove kick-warped 0.05");
    p.ok("clip warp remove kick-warped 0.05");
    assert_eq!(markers(&p, "kick-warped"), 2);
    p.refused("clip warp remove kick-warped 0.1", "needs two points");
    p.refused("clip warp remove kick 0.1", "no stretch map to remove a point from");
    p.refused("clip warp kick-warped -1 2", "counted from zero");
}

#[test]
fn a_clip_with_no_map_is_seeded_from_its_length_and_its_file() {
    let mut p = Project::open("clip-lane");
    p.round_trip("clip warp kick 0.05 0.1");
    p.ok("clip warp kick 0.05 0.1");
    assert_eq!(markers(&p, "kick"), 3);
    assert!(p.text("clips/kick.k7x").contains("elastique"));
    p.refused("clip warp nowhere 0.05 0.1", "no clip \"nowhere\"");
}

#[test]
fn onsets_are_corrections_that_cancel_out() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("clip onset add kick 0.05");
    assert_eq!(ran.answer.log, ["kick: 1 added, 0 removed"]);
    p.ok("clip onset add kick 0.05");
    assert!(p.text("clips/kick.k7x").contains("onsets"));
    let back = p.ok("clip onset remove kick 0.05");
    assert_eq!(back.answer.log, ["kick: onsets back to what the engine detects"]);
    assert!(!p.text("clips/kick.k7x").contains("onsets"));
    let moved = p.round_trip("clip onset move kick 0.02 0.08");
    assert_eq!(moved.answer.log, ["kick: 1 added, 1 removed"]);
    // Within the match tolerance (25 ms), a move lands back on the onset it took away.
    let nudged = p.ok("clip onset move kick 0.02 0.03");
    assert_eq!(nudged.answer.log, ["kick: onsets back to what the engine detects"]);
}

#[test]
fn onset_refusals() {
    let mut p = Project::open("clip-lane");
    p.refused("clip onset add nowhere 0.1", "clip onset add:");
    p.refused("clip onset move kick 0.02", "usage: clip onset move");
}

#[test]
fn a_crossfade_overlaps_two_hits_and_zero_takes_the_fades_away() {
    let mut p = Project::open("clip-lane");
    p.ok("project tempo 400");
    p.ok("event window drums 1:2:1 0.06 0.1");
    let ran = p.round_trip("event crossfade drums 1:1:1 1:2:1 0:0:1");
    assert!(
        ran.answer.log[0].starts_with("drums: crossfade of 0.07 beat(s) between 1:1:1 and "),
        "{:?}",
        ran.answer.log
    );
    p.ok("event crossfade drums 1:1:1 1:2:1 0:0:1");
    let clips = p.track("drums").clips;
    let (kick, snare) = (&clips[0], &clips[1]);
    assert!(
        snare.start < 1.0 && kick.fade_out > 0.0 && snare.fade_in > 0.0,
        "{} {} {}",
        snare.start,
        kick.fade_out,
        snare.fade_in
    );
    // The kick plays its whole 0.1 s file, 2/3 of a beat at 400 bpm: B now starts inside it.
    assert!(snare.start < 0.1 * 400.0 / 60.0, "they overlap");
    p.refused("event crossfade drums 1:1:1 1:1:1 0:0:1", "no two events");
    p.refused("event crossfade drums 1:1:1 1:2:1", "usage: event crossfade");
    let mut q = Project::open("clip-lane");
    q.refused(
        "event crossfade drums 1:1:1 1:2:1 0:0:1",
        "neither event has material to overlap with",
    );
    q.ok("event fadeout drums 1:1:1 0:0:1");
    q.ok("event crossfade drums 1:1:1 1:2:1 0");
    assert_eq!(q.track("drums").clips[0].fade_out, 0.0);
    let mut seq = Project::open("edit-undo");
    seq.refused("event crossfade melody 1:1:1 1:1:1 0:0:1", "no two events");
}

#[test]
fn a_sequence_range_is_consolidated_and_audio_wants_a_render() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("selection consolidate melody 1:1:1 2:1");
    assert_eq!(ran.answer.log[1], "  4 notes, from melody");
    p.ok("selection consolidate melody 1:1:1 2:1");
    let clips = p.track("melody").clips;
    assert_eq!(clips.len(), 1);
    assert_ne!(clips[0].name, "melody", "the event now plays the new sequence");
    assert_eq!(clips[0].notes.len(), 4);
    p.refused("selection consolidate melody,lead 1:1:1 2:1", "takes ONE track");
    let mut a = Project::open("clip-lane");
    a.refused("selection consolidate drums 1:1:1 2:1", "no engine to render with");
}

#[test]
fn a_render_or_an_envelope_is_refused_by_the_member_it_needs() {
    let mut p = Project::open("clip-lane");
    p.refused("event trim drums 1:1:1 -40", "(clipEnvelope)");
    p.refused("event trim drums 1:1:1 6", "is not a gate in dBFS");
    p.refused("event trim drums 9:1:1", "no event on 'drums'");
    p.refused("event process drums 1:1:1 @k7-core/amp-room", "(render)");
    p.refused("event process drums 1:3:2 @k7-core/amp-room", "no event on 'drums'");
    p.refused("clip process kick @k7-core/amp-room", "(render)");
    p.refused("track process drums @k7-core/amp-room", "(render)");
    p.refused("track process drums", "usage: track process");
    let mut s = Project::open("edit-undo");
    s.refused("event process melody 1:1:1 @k7-core/amp-room", "is not audio");
}
