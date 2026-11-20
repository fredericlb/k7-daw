// clips_comp.rs — a comped event compiled to plain clips, the slice a take is recorded into, and the crossfades two clips make.
// A reader would get wrong that one slice is two units: AUDIO moves its window into the FILE (seconds), MIDI over the NOTES (beats).

use crate::clips_support;

use clips_support::{event, grid, lane, world};
use serde_json::{Value, json};
use song::clips::{BUDGET, Budget, ClipDoc, ClipKind, comp_clips, cut_hole, mark_crossfades, slice_clip};

// 120 bpm: one beat is half a second.
const TEMPO: f64 = 120.0;

fn audio(name: &str, start: f64, length: f64) -> ClipDoc {
    serde_json::from_value(json!({
        "id": format!("{name}@{start}"), "type": "audio", "name": name, "source": format!("/clips/{name}.wav"),
        "start": start, "length": length, "offset": 0, "sourceFrom": 0, "sourceTo": length / 2.0,
        "fadeIn": 1, "fadeOut": 1, "loopStart": 0, "loopEnd": 0, "notes": []
    }))
    .expect("a clip")
}

fn comp(p: Value, own: &ClipDoc, take: &ClipDoc) -> Vec<ClipDoc> {
    let take = take.clone();
    comp_clips(
        &event(p),
        own.clone(),
        &grid(TEMPO),
        &mut Budget::new(BUDGET),
        &mut |_, _, _| Some(take.clone()),
    )
    .expect("clips")
}

#[test]
fn an_audio_slice_moves_its_window_into_the_file_in_seconds_and_drops_the_events_fades() {
    // compClips.test.ts "moves an audio window into the file, in seconds, and drops the event's fades"
    let s = slice_clip(&audio("gtr", 16.0, 8.0), 2.0, Some(2.0), 18.0, &grid(TEMPO));
    assert_eq!(
        (s.start, s.length, s.source_from, s.source_to),
        (18.0, 2.0, Some(1.0), Some(2.0))
    );
    assert_eq!((s.fade_in, s.fade_out), (0.0, 0.0));
    assert_eq!(s.id, "gtr@18");
}

#[test]
fn a_midi_slice_moves_its_window_over_the_notes_in_beats() {
    // compClips.test.ts "moves a MIDI window over the notes, in beats"
    let mut seq = audio("keys", 0.0, 8.0);
    seq.kind = ClipKind::Midi;
    seq.source = String::new();
    let s = slice_clip(&seq, 4.0, Some(2.0), 4.0, &grid(TEMPO));
    assert_eq!((s.start, s.length, s.loop_start, s.loop_end), (4.0, 2.0, 4.0, 6.0));
}

#[test]
fn a_slice_drops_the_stretch_map_and_keeps_the_declined_flag() {
    let mut c = audio("gtr", 0.0, 8.0);
    c.warp = Some(song::clips::WarpDoc {
        algorithm: "slice".into(),
        markers: vec![(0.0, 0.0), (4.0, 2.0)],
    });
    c.warp_off = Some(true);
    let s = slice_clip(&c, 1.0, None, 1.0, &grid(TEMPO));
    assert!(s.warp.is_none() && s.warp_off == Some(true));
    // to its end: the window's end is the clip's own
    assert_eq!(s.source_to, c.source_to);
}

#[test]
fn the_take_plays_over_its_span_and_the_events_own_material_around_it_each_piece_marked() {
    // compClips.test.ts "plays the take over its span and the event's own material around it"
    let (own, take) = (audio("gtr", 16.0, 8.0), audio("gtr_t2", 16.0, 12.0));
    let clips = comp(
        json!({"ref": "gtr", "start": "5:1", "takes": [{"ref": "gtr_t2", "start": "5:1"}], "comp": [{"take": "gtr_t2", "from": "5:3", "to": "8:1"}]}),
        &own,
        &take,
    );
    let got: Vec<(&str, f64, f64)> = clips.iter().map(|c| (c.name.as_str(), c.start, c.length)).collect();
    assert_eq!(got, [("gtr", 16.0, 2.0), ("gtr_t2", 18.0, 10.0)]);
    assert!(clips.iter().all(|c| c.comp.as_ref().is_some_and(|m| m.of == own.id)));
    // the event's own fades belong to its two ends: fadeIn at the first start, fadeOut at the last end
    assert_eq!(
        (clips[0].fade_in, clips[0].fade_out, clips[1].fade_in, clips[1].fade_out),
        (1.0, 0.0, 0.0, 1.0)
    );
}

#[test]
fn an_event_with_no_comp_or_a_looped_one_is_its_one_clip() {
    // compClips.test.ts "leaves an event with no comp, or a looped one, as its one clip"
    let own = audio("gtr", 16.0, 8.0);
    assert_eq!(comp(json!({"ref": "gtr"}), &own, &own), vec![own.clone()]);
    let looped = json!({"ref": "gtr", "loop": 2, "comp": [{"take": "x", "from": "5:1", "to": "6:1"}]});
    assert_eq!(comp(looped, &own, &own), vec![own]);
}

#[test]
fn a_muted_comped_event_mutes_every_piece_takes_included() {
    // decision 160: takeClip compiles a take without the event's words, so the mute is laid on every piece
    let (mut own, take) = (audio("gtr", 16.0, 8.0), audio("gtr_t2", 16.0, 12.0));
    own.disabled = Some(true);
    let p = json!({"ref": "gtr", "start": "5:1", "disabled": true, "takes": [{"ref": "gtr_t2", "start": "5:1"}], "comp": [{"take": "gtr_t2", "from": "5:3", "to": "8:1"}]});
    let clips = comp(p, &own, &take);
    assert_eq!(clips.len(), 2);
    assert!(clips.iter().all(|c| c.disabled == Some(true)));
}

#[test]
fn the_events_gain_stays_on_its_own_materials_pieces() {
    // decision 161: as the TypeScript, a take's pieces carry no trim
    let (mut own, take) = (audio("gtr", 16.0, 8.0), audio("gtr_t2", 16.0, 12.0));
    own.gain_db = Some(3.0);
    let p = json!({"ref": "gtr", "start": "5:1", "gain": 3, "takes": [{"ref": "gtr_t2", "start": "5:1"}], "comp": [{"take": "gtr_t2", "from": "5:3", "to": "6:1"}]});
    let clips = comp(p, &own, &take);
    assert_eq!(
        clips.iter().map(|c| c.gain_db).collect::<Vec<_>>(),
        [Some(3.0), None, Some(3.0)]
    );
}

#[test]
fn an_unknown_take_silences_its_span_and_an_all_silent_comp_makes_no_clip() {
    let own = audio("gtr", 16.0, 8.0);
    let nope = json!({"ref": "gtr", "start": "5:1", "comp": [{"take": "nope", "from": "5:3", "to": "6:1"}]});
    assert_eq!(comp(nope, &own, &own).len(), 2);
    let all = json!({"ref": "gtr", "start": "5:1", "comp": [{"take": "nope", "from": "1:1", "to": "99:1"}]});
    assert!(comp(all, &own, &own).is_empty());
}

#[test]
fn a_take_of_the_events_own_ref_replaces_its_clip() {
    // `clipOf`'s own entry is overwritten by a take of the same ref
    let (own, other) = (audio("gtr", 16.0, 8.0), audio("gtr", 16.0, 4.0));
    let p = json!({"ref": "gtr", "start": "5:1", "takes": [{"ref": "gtr", "start": "5:1"}], "comp": [{"take": "gtr", "from": "5:3", "to": "6:1"}]});
    let clips = comp(p, &own, &other);
    assert!(clips.iter().all(|c| c.source_to.is_some_and(|t| t <= 2.0 + 1e-9)));
}

#[test]
fn a_seam_crossfade_is_marked_on_audio_and_ignored_on_midi() {
    let (own, take) = (audio("gtr", 16.0, 8.0), audio("gtr_t2", 16.0, 12.0));
    let p = json!({"ref": "gtr", "start": "5:1", "takes": [{"ref": "gtr_t2", "start": "5:1"}],
        "comp": [{"take": "gtr_t2", "from": "5:3", "to": "6:1"}], "crossfades": [{"at": "5:3", "length": "0:1"}]});
    let clips = comp(p.clone(), &own, &take);
    assert_eq!(clips[0].crossfade.and_then(|m| m.out), Some(true));
    assert_eq!(clips[1].crossfade.and_then(|m| m.in_), Some(true));
    assert_eq!((clips[0].fade_out, clips[1].fade_in), (1.0, 1.0));
    let (mut seq, mut take) = (own.clone(), take.clone());
    seq.kind = ClipKind::Midi;
    take.kind = ClipKind::Midi;
    assert!(comp(p, &seq, &take).iter().all(|c| c.crossfade.is_none()));
}

#[test]
fn a_comp_on_a_sequence_slices_over_its_notes() {
    let notes = json!([{"t": 0, "d": 1, "k": 60}]);
    let m = world(&[], &[("riff", json!({"notes": notes}))], &[]);
    let p = json!({"ref": "riff", "start": "1:1", "takes": [{"ref": "riff", "start": "1:1"}], "comp": [{"take": "riff", "from": "1:3", "to": "2:1"}]});
    let clips = lane(&m, TEMPO, &[p]);
    assert!(!clips.is_empty() && clips.iter().all(|c| c.kind == ClipKind::Midi));
}

#[test]
fn cutting_a_hole_silences_a_span_keeping_both_sides() {
    // compClips.test.ts "cutHole silences a span, keeping both sides"
    let clips = cut_hole(&[audio("gtr", 16.0, 8.0)], 18.0, 20.0, &grid(TEMPO));
    let got: Vec<(f64, f64, Option<f64>)> = clips.iter().map(|c| (c.start, c.length, c.source_from)).collect();
    assert_eq!(got, [(16.0, 2.0, Some(0.0)), (20.0, 4.0, Some(2.0))]);
}

#[test]
fn a_clip_outside_the_hole_is_left_alone_and_one_with_no_length_reaches_past_it() {
    let (a, mut open) = (audio("a", 0.0, 4.0), audio("b", 10.0, 0.0));
    open.source_to = None;
    let got = cut_hole(&[a.clone(), open], 12.0, 14.0, &grid(TEMPO));
    assert_eq!(got[0], a);
    assert_eq!(got.len(), 3);
    assert_eq!(got[2].length, 0.0);
}

fn faded(name: &str, start: f64, fade_in: f64, fade_out: f64) -> ClipDoc {
    ClipDoc {
        fade_in,
        fade_out,
        ..audio(name, start, 8.0)
    }
}

#[test]
fn two_clips_overlapping_by_exactly_their_two_fades_are_a_crossfade_and_nothing_else_is() {
    // compClips.test.ts "marks two clips overlapping by exactly their two fades, and nothing else"
    let clips = vec![
        ClipDoc {
            length: 8.0,
            ..faded("a", 0.0, 0.0, 1.0)
        },
        faded("b", 7.0, 1.0, 0.0),
        ClipDoc {
            length: 4.0,
            ..faded("c", 20.0, 1.0, 0.0)
        },
    ];
    let got = mark_crossfades(clips, &mut Budget::new(BUDGET));
    assert_eq!(got[0].crossfade.and_then(|m| m.out), Some(true));
    assert_eq!(got[1].crossfade.and_then(|m| m.in_), Some(true));
    assert!(got[2].crossfade.is_none());
    // the order is the input's
    assert_eq!(got.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
}

#[test]
fn only_audio_with_a_length_is_marked_and_a_midi_overlap_gets_none() {
    let mut a = faded("a", 0.0, 0.0, 1.0);
    let mut b = faded("b", 7.0, 1.0, 0.0);
    a.kind = ClipKind::Midi;
    b.kind = ClipKind::Midi;
    assert!(
        mark_crossfades(vec![a, b], &mut Budget::new(BUDGET))
            .iter()
            .all(|c| c.crossfade.is_none())
    );
}

#[test]
fn a_dangling_ref_makes_a_comp_of_nothing_to_slice() {
    // placement through the lane: a comp over a ref nothing holds still returns the empty clip
    let m = world(&[], &[], &[]);
    let clips = lane(
        &m,
        TEMPO,
        &[json!({"ref": "ghost", "start": "1:1", "comp": [{"take": "ghost", "from": "1:1", "to": "2:1"}]})],
    );
    assert_eq!(clips.len(), 2);
}
