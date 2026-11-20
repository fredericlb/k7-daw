// clips_placement.rs — one placement read into the ClipDoc the engine plays: windows, loops, fades, gain, warp, the event's words.
// A reader would get wrong the units: `start` and `loop` are positions, the rest durations, and a NUMBER window edge is seconds.

use crate::clips_support;

use clips_support::{grid, lane, one, world};
use serde_json::{Value, json};
use song::clips::{ClipDoc, ClipKind, Materials, Placer};
use song::files::WindowEdge;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} is not {b}");
}

fn kick(clip: Value) -> Materials {
    world(&[("kick", clip)], &[], &[])
}

fn at(mats: &Materials, event: Value) -> ClipDoc {
    one(mats, 120.0, event).expect("a clip")
}

fn ev(extra: Value) -> Value {
    let mut base = json!({ "ref": "kick", "start": "1:1:1" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    base
}

#[test]
fn a_clip_windows_every_placement_of_it() {
    // clipWindow.test.ts "compiles from/to onto every placement of the clip"
    let m = kick(json!({"source": "clips/kick.wav", "from": 0.25, "to": 1.5}));
    let c = at(&m, ev(json!({})));
    assert_eq!((c.source_from, c.source_to), (Some(0.25), Some(1.5)));
    assert_eq!(c.source, "/p/clips/kick.wav");
}

#[test]
fn an_untrimmed_clip_has_no_window_at_all() {
    // clipWindow.test.ts "leaves them off entirely when the clip trims nothing"
    let c = at(&kick(json!({"source": "clips/kick.wav"})), ev(json!({})));
    assert_eq!((c.source_from, c.source_to), (None, None));
}

#[test]
fn one_edge_is_just_one_edge() {
    // clipWindow.test.ts "takes just one edge"
    let c = at(&kick(json!({"source": "clips/kick.wav", "from": 0.1})), ev(json!({})));
    assert_eq!((c.source_from, c.source_to), (Some(0.1), None));
}

#[test]
fn the_window_gives_the_clip_its_extent_in_beats() {
    // clipWindow.test.ts "gives the clip its extent in beats": 120 bpm, a beat is half a second
    close(
        at(&kick(json!({"source": "k.wav", "to": 1.5})), ev(json!({}))).length,
        3.0,
    );
    close(
        at(&kick(json!({"source": "k.wav", "from": 0.5, "to": 1.5})), ev(json!({}))).length,
        2.0,
    );
}

#[test]
fn only_the_file_knows_the_extent_without_a_to() {
    // clipWindow.test.ts "leaves the extent at 0 when only the FILE knows it", and "does not disturb where the placement puts it"
    let c = at(&kick(json!({"source": "k.wav", "from": 0.25})), ev(json!({})));
    assert_eq!((c.start, c.length, c.offset), (0.0, 0.0, 0.0));
}

#[test]
fn the_events_own_window_narrows_the_clips_and_the_most_restrictive_edge_wins() {
    // clipWindow.test.ts "windows the placement when the clip states none", "intersects with the clip's"
    let m = kick(json!({"source": "k.wav"}));
    let c = at(&m, ev(json!({"from": 0.25, "to": 1.5})));
    assert_eq!((c.source_from, c.source_to), (Some(0.25), Some(1.5)));
    close(c.length, 2.5);
    let m = kick(json!({"source": "k.wav", "from": 0.5, "to": 1.25}));
    let c = at(&m, ev(json!({"from": 0.25, "to": 1.5})));
    assert_eq!((c.source_from, c.source_to), (Some(0.5), Some(1.25)));
    let m = kick(json!({"source": "k.wav", "from": 0.25, "to": 1.5}));
    let c = at(&m, ev(json!({"from": 0.5, "to": 1.25})));
    assert_eq!((c.source_from, c.source_to), (Some(0.5), Some(1.25)));
    let c = at(&m, ev(json!({"to": 1})));
    assert_eq!((c.source_from, c.source_to), (Some(0.25), Some(1.0)));
}

#[test]
fn a_timecode_edge_is_a_position_in_the_file() {
    // clipWindow.test.ts "reads a timecode edge like the clip's own", "reads as a POSITION in the file", "puts 1:1 at the file's very start"
    let m = kick(json!({"source": "k.wav"}));
    close(at(&m, ev(json!({"from": "2:1"}))).source_from.expect("from"), 2.0);
    let m = kick(json!({"source": "k.wav", "from": "2:1"}));
    let c = at(&m, ev(json!({})));
    close(c.source_from.expect("from"), 2.0);
    assert_eq!(c.source_to, None);
    let m = kick(json!({"source": "k.wav", "from": "1:1"}));
    assert_eq!(at(&m, ev(json!({}))).source_from, None);
    // the event's words keep `from: 0` even when the edge is absent
    let m = kick(json!({"source": "k.wav"}));
    let c = at(&m, ev(json!({"from": 0})));
    assert_eq!(c.source_from, None);
    assert_eq!(c.event.expect("event").from, Some(WindowEdge::Seconds(0.0)));
}

#[test]
fn seconds_and_timecodes_mix_per_edge() {
    // clipWindow.test.ts "mixes with seconds"
    let m = kick(json!({"source": "k.wav", "from": 0.5, "to": "2:1"}));
    let c = at(&m, ev(json!({})));
    close(c.source_from.expect("from"), 0.5);
    close(c.source_to.expect("to"), 2.0);
}

#[test]
fn an_events_window_reaches_that_event_alone() {
    // clipWindow.test.ts "does not reach the OTHER placements of the clip"
    let m = world(
        &[
            ("kick", json!({"source": "k.wav"})),
            ("snare", json!({"source": "s.wav"})),
        ],
        &[],
        &[],
    );
    let clips = lane(
        &m,
        120.0,
        &[ev(json!({"from": 0.25})), json!({"ref": "snare", "start": "2:1:1"})],
    );
    assert_eq!(clips[0].source_from, Some(0.25));
    assert_eq!(clips[1].source_from, None);
}

#[test]
fn a_start_in_seconds_goes_through_the_tempo() {
    // secondsStart.test.ts "places a number of seconds through the tempo, beside a musical start"
    let m = kick(json!({"source": "k.wav"}));
    let clips = lane(
        &m,
        120.0,
        &[
            json!({"ref": "kick", "start": "2:1"}),
            json!({"ref": "kick", "start": 4.5}),
        ],
    );
    assert_eq!(clips.iter().map(|c| c.start).collect::<Vec<_>>(), [4.0, 9.0]);
    // secondsStart.test.ts "does not move a seconds start when the tempo changes": 3 s at 60 bpm is beat 3
    let c = one(&m, 60.0, json!({"ref": "kick", "start": 3})).expect("a clip");
    assert_eq!(c.start, 3.0);
}

#[test]
fn what_only_a_shot_reads_is_refused_by_event_and_the_rest_compile() {
    // secondsStart.test.ts "refuses what only a shot reads on an event that sounds, by name"; decision 155
    let m = kick(json!({"source": "k.wav"}));
    let mut placer = Placer::new(grid(120.0), &m);
    let bad = support_event(json!({"ref": "kick", "start": 1, "length": 2, "fadeIn": 0.5}));
    assert!(placer.placement_to_clip(&bad, "nodes/fx.k7x").is_none());
    let again = support_event(json!({"ref": "kick", "start": 1, "length": 3}));
    assert!(placer.placement_to_clip(&again, "nodes/fx.k7x").is_none());
    let fine = support_event(json!({"ref": "kick", "start": 2}));
    assert!(placer.placement_to_clip(&fine, "nodes/fx.k7x").is_some());
    let (findings, capped, _) = placer.finish();
    assert!(!capped);
    // one per event, each named by the lane's file at 1:1; the ref and the start stay in the message
    assert_eq!(findings.len(), 2);
    assert!(
        findings
            .iter()
            .all(|f| f.file == "nodes/fx.k7x" && (f.line, f.col) == (1, 1))
    );
    assert!(findings[0].message.contains("'kick' at 1s"));
}

fn support_event(v: Value) -> song::Placement {
    clips_support::event(v)
}

#[test]
fn a_shot_key_a_map_fade_or_no_ref_is_refused_and_a_repeat_is_kept_once() {
    // decision 155: a shot's key, a transition-map fade, and an event with no ref
    let m = kick(json!({"source": "k.wav"}));
    let mut placer = Placer::new(grid(120.0), &m);
    for e in [
        json!({"ref": "kick", "start": "1:1", "fadeIn": {"over": 1.0}}),
        json!({"ref": "kick", "start": "1:1", "speed": 2}),
        json!({"start": "1:1"}),
        json!({"ref": "kick", "start": "1:1", "speed": 2}),
    ] {
        assert!(placer.placement_to_clip(&support_event(e), "nodes/a.k7x").is_none());
    }
    let (findings, _, _) = placer.finish();
    assert_eq!(findings.len(), 3);
}

#[test]
fn an_events_words_are_carried_unmerged_and_only_what_was_said() {
    // eventAttrs.test.ts "carries every field the event stated"
    let m = kick(json!({"source": "k.wav", "to": 8}));
    let c = at(
        &m,
        ev(
            json!({"from": 1.5, "to": 6.5455, "fadeIn": "0:2", "fadeOut": "0:1", "loop": 3, "disabled": true, "warp": false}),
        ),
    );
    let got = serde_json::to_value(c.event.expect("event")).expect("json");
    assert_eq!(
        got,
        json!({"from": 1.5, "to": 6.5455, "fadeIn": "0:2", "fadeOut": "0:1", "loop": 3, "disabled": true, "warp": false})
    );
}

#[test]
fn a_timecode_window_is_spelled_as_written_beside_the_resolved_one() {
    // eventAttrs.test.ts "keeps a timecode window spelled as a timecode", "sits beside the resolved window"
    let m = kick(json!({"source": "k.wav", "to": 8}));
    let c = at(&m, ev(json!({"from": "2:1", "to": "4:1"})));
    let e = c.event.as_ref().expect("event");
    assert_eq!(e.from, Some(WindowEdge::Timecode("2:1".into())));
    assert_eq!(e.to, Some(WindowEdge::Timecode("4:1".into())));
    close(c.source_from.expect("from"), 2.0);
}

#[test]
fn an_event_that_stated_nothing_or_only_defaults_has_no_event() {
    // eventAttrs.test.ts "is absent on an event that stated nothing", "leaves out the values that mean the default"
    let m = kick(json!({"source": "k.wav", "to": 8}));
    assert!(at(&m, ev(json!({}))).event.is_none());
    assert!(at(&m, ev(json!({"loop": 1, "warp": true}))).event.is_none());
}

#[test]
fn a_pads_effects_ride_on_the_note_unexpanded() {
    // padEffects.test.ts "compiling a pad's effects": puts them on the note UNEXPANDED, says nothing without, ONE note per pad
    let seq = |note: Value| world(&[], &[("pat", json!({"notes": [note]}))], &[]);
    let c = at(
        &seq(json!({"t": 0, "d": 0.25, "k": 36, "repeat": 4, "skip": 2})),
        json!({"ref": "pat", "start": "1:1:1"}),
    );
    let n = &c.notes[0];
    assert_eq!((n.repeat, n.skip, n.duration), (Some(4), Some(2), 0.25));
    assert_eq!(c.notes.len(), 1);
    let c = at(
        &seq(json!({"t": 0, "d": 0.25, "k": 36})),
        json!({"ref": "pat", "start": "1:1:1"}),
    );
    let n = &c.notes[0];
    assert_eq!((n.repeat, n.skip, n.vel, n.channel, n.rel), (None, None, 1.0, 0, 1.0));
}

#[test]
fn two_placements_of_one_material_get_different_ids() {
    // placementId.test.ts "get DIFFERENT ids", "still NAME the material", "carries the start, so the id says WHERE"
    let m = kick(json!({"source": "k.wav"}));
    let clips = lane(
        &m,
        120.0,
        &[
            json!({"ref": "kick", "start": "1:1:1", "to": 2}),
            json!({"ref": "kick", "start": "2:1:1", "from": 2}),
        ],
    );
    let ids: Vec<&str> = clips.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["kick@0", "kick@4"]);
    assert!(clips.iter().all(|c| c.name == "kick"));
    assert_eq!(at(&m, json!({"ref": "kick", "start": "3:1:1"})).id, "kick@8");
}

fn map_clip(extra: Value) -> Materials {
    let mut c =
        json!({"source": "clips/riff.wav", "to": "2:1", "warp": {"algorithm": "slice", "markers": [[0, 0], [4, 2]]}});
    for (k, v) in extra.as_object().expect("object") {
        c["warp"][k] = v.clone();
    }
    world(&[("riff", c)], &[], &[])
}

#[test]
fn a_clips_warp_reaches_every_placement_unless_it_is_declined() {
    // warpDisable.test.ts "a clip's warp" (three)
    let place = json!({"ref": "riff", "start": "1:1:1"});
    let c = at(&map_clip(json!({})), place.clone());
    assert_eq!(
        c.warp.as_ref().map(|w| (w.algorithm.as_str(), w.markers.len())),
        Some(("slice", 2))
    );
    assert_eq!(c.warp_off, None);
    let c = at(&map_clip(json!({"disable": true})), place.clone());
    assert_eq!((c.warp, c.warp_off), (None, Some(true)));
    let c = at(&map_clip(json!({"disable": false})), place);
    assert!(c.warp.is_some() && c.warp_off.is_none());
}

#[test]
fn an_event_that_declines_the_map_says_so_only_when_there_is_one() {
    // the brief: `warp_off` only when there IS a map and it is declined; `warp: false` on no map says nothing
    let c = at(
        &map_clip(json!({})),
        json!({"ref": "riff", "start": "1:1:1", "warp": false}),
    );
    assert_eq!((c.warp, c.warp_off), (None, Some(true)));
    let c = at(&kick(json!({"source": "k.wav"})), ev(json!({"warp": false})));
    assert_eq!((c.warp, c.warp_off), (None, None));
}

#[test]
fn a_loop_repeats_the_window_in_the_source_and_a_gain_is_clamped() {
    let m = kick(json!({"source": "k.wav", "from": 0.5, "to": 1.5}));
    let c = at(&m, ev(json!({"loop": 3, "gain": 9.0e-1})));
    close(c.length, 6.0);
    close(c.loop_start, 1.0);
    close(c.loop_end, 3.0);
    assert_eq!(c.gain_db, Some(0.9));
    assert_eq!(c.event.expect("event").loop_, Some(3));
    // no window to loop: nothing to repeat, length stays 0
    let c = at(&kick(json!({"source": "k.wav"})), ev(json!({"loop": 4})));
    assert_eq!((c.length, c.loop_start, c.loop_end), (0.0, 0.0, 0.0));
    // 0 is unity and sent as nothing
    assert_eq!(
        at(&kick(json!({"source": "k.wav"})), ev(json!({"gain": 0}))).gain_db,
        None
    );
}

#[test]
fn fades_are_durations() {
    let c = at(
        &kick(json!({"source": "k.wav"})),
        ev(json!({"fadeIn": "0:2", "fadeOut": "1:0"})),
    );
    assert_eq!((c.fade_in, c.fade_out), (2.0, 4.0));
}

#[test]
fn a_sequence_defaults_to_its_notes_extent_rounded_up_to_the_bar() {
    let notes = json!([{"t": 0, "d": 1, "k": 60}, {"t": 4.5, "d": 1, "k": 62, "v": 0.5}]);
    let m = world(&[], &[("riff", json!({"notes": notes}))], &[]);
    let c = at(&m, json!({"ref": "riff", "start": "1:1:1"}));
    assert_eq!(c.kind, ClipKind::Midi);
    assert_eq!((c.length, c.loop_start, c.loop_end), (8.0, 0.0, 0.0));
    assert_eq!(c.notes[1].vel, 0.5);
    // looped twice: a loop range over the whole extent
    let c = at(&m, json!({"ref": "riff", "start": "1:1:1", "loop": 2}));
    assert_eq!((c.length, c.loop_start, c.loop_end), (16.0, 0.0, 8.0));
}

#[test]
fn a_sequences_own_window_falls_back_and_the_events_has_no_fallback() {
    let notes = json!([{"t": 0, "d": 1, "k": 60}]);
    let seq = |extra: Value| {
        let mut s = json!({ "notes": notes });
        for (k, v) in extra.as_object().expect("object") {
            s[k] = v.clone();
        }
        world(&[], &[("riff", s)], &[])
    };
    let place = |extra: Value| {
        let mut e = json!({"ref": "riff", "start": "1:1:1"});
        for (k, v) in extra.as_object().expect("object") {
            e[k] = v.clone();
        }
        e
    };
    // `to` before `from` falls back to the extent (one bar: 4 beats)
    let c = at(&seq(json!({"from": "2:1", "to": "1:1"})), place(json!({})));
    assert_eq!((c.length, c.loop_start, c.loop_end), (0.0, 0.0, 0.0));
    // a stated `to` off the extent cuts a tail and is worth a loop range
    let c = at(&seq(json!({"to": "1:3"})), place(json!({})));
    assert_eq!((c.length, c.loop_start, c.loop_end), (2.0, 0.0, 2.0));
    // the event's window narrows, with no fallback: `to` at the start leaves nothing
    let c = at(&seq(json!({})), place(json!({"from": "1:3", "to": "1:2"})));
    assert_eq!(c.length, 0.0);
    let c = at(&seq(json!({})), place(json!({"from": "1:3"})));
    assert_eq!((c.length, c.loop_start, c.loop_end), (2.0, 2.0, 4.0));
}

#[test]
fn a_dangling_ref_is_an_empty_audio_clip() {
    let c = at(&world(&[], &[], &[]), json!({"ref": "ghost", "start": "2:1:1"}));
    assert_eq!(
        (c.kind, c.source.as_str(), c.name.as_str(), c.id.as_str()),
        (ClipKind::Audio, "", "ghost", "ghost@4")
    );
}

#[test]
fn a_video_clip_on_a_sound_lane_is_an_audio_clip_of_its_source() {
    // decision 163
    let m = world(&[("shot", json!({"source": "clips/shot.mp4"}))], &[], &[]);
    let c = at(&m, json!({"ref": "shot", "start": "1:1:1", "warp": false}));
    assert_eq!((c.kind, c.source.as_str()), (ClipKind::Audio, "/p/clips/shot.mp4"));
    assert!(c.warp.is_none() && c.warp_off.is_none() && c.source_from.is_none());
}

#[test]
fn a_disabled_event_is_carried_not_filtered() {
    let c = at(&kick(json!({"source": "k.wav"})), ev(json!({"disabled": true})));
    assert_eq!(c.disabled, Some(true));
    assert_eq!(c.event.expect("event").disabled, Some(true));
}

#[test]
fn a_number_that_is_not_finite_drops_the_clip_with_an_error() {
    // decision 156: a 400-digit start, a window past what the tempo can hold
    let m = kick(json!({"source": "k.wav", "to": 1.0e308}));
    let digits = format!("{}:1", "9".repeat(400));
    let mut placer = Placer::new(grid(300.0), &m);
    let huge = clips_support::event(json!({"ref": "kick", "start": digits}));
    assert!(placer.placement_to_clip(&huge, "nodes/x.k7x").is_none());
    let wide = clips_support::event(json!({"ref": "kick", "start": "1:1"}));
    assert!(placer.placement_to_clip(&wide, "nodes/x.k7x").is_none());
    let (findings, _, _) = placer.finish();
    assert_eq!(findings.len(), 2);
    let mut tiny = Placer::new(grid(1e-300), &m);
    // a timecode edge in seconds is (beats * 60) / tempo, which a tempo of 1e-300 sends past a double
    let far = clips_support::event(json!({"ref": "kick", "start": "1:1", "from": "999999:1"}));
    assert!(tiny.placement_to_clip(&far, "nodes/x.k7x").is_none());
    let near = clips_support::event(json!({"ref": "kick", "start": "1:1"}));
    assert!(tiny.placement_to_clip(&near, "nodes/x.k7x").is_some());
}

#[test]
fn a_loop_near_two_to_the_53_is_a_finite_count() {
    // clips_hostile's neighbour: a huge pass count on a window and with none stays finite or is refused, never a panic
    let m = kick(json!({"source": "k.wav", "to": 1.5}));
    let big = (1_u64 << 53) - 1;
    let c = at(&m, ev(json!({"loop": big})));
    assert!(c.length.is_finite() && c.event.expect("event").loop_ == Some(big));
    assert!(one(&kick(json!({"source": "k.wav"})), 120.0, ev(json!({"loop": big}))).is_some());
}

#[test]
fn the_budget_stops_a_pass_and_names_the_file_it_stopped_in() {
    // decision 157, at a small limit
    let m = kick(json!({"source": "k.wav"}));
    let mut placer = Placer::with_limit(grid(120.0), &m, 3);
    let e = clips_support::event(json!({"ref": "kick", "start": "1:1"}));
    let got: Vec<bool> = (0..6)
        .map(|_| placer.placement_to_clip(&e, "nodes/z.k7x").is_some())
        .collect();
    assert_eq!(got, [true, true, true, false, false, false]);
    let (findings, capped, weight) = placer.finish();
    assert!(capped && weight > 3);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].file, "nodes/z.k7x");
}
