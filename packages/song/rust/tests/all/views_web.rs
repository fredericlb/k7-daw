// views_web.rs — a clip as the browser reads it and the two folds, after webTracks, webClipLength, webComp and webNest tests.
// A reader would get wrong `length`: 0 is "unstated", and only an audio clip with a source and a tempo is filled in.

use crate::views_support;

use serde_json::{Value, json};
use song::views::web::{WebClip, fold_comps, fold_nests, web_clip, web_clips};
use views_support::{Counting, clip, fast};

fn drawn(over: Value, tempo: Option<f64>, m: &mut Counting) -> WebClip {
    web_clip(&clip(over), tempo, m).expect("reads")
}

fn json_of(c: &WebClip) -> Value {
    serde_json::to_value(c).expect("json")
}

fn table(path: &str, secs: f64) -> Counting {
    let mut m = Counting::default();
    m.table.insert(path.to_owned(), secs);
    m
}

fn beats(secs: f64, tempo: f64) -> f64 {
    (secs * tempo) / 60.0
}

#[test]
fn a_clip_with_no_stated_extent_is_drawn_at_its_sources_length() {
    // webClipLength.test.ts: is drawn at its SOURCE's length (615).
    let mut m = table("/a/r.wav", 6.716938);
    let w = drawn(json!({"source": "/a/r.wav"}), Some(110.0), &mut m);
    assert!((w.length - beats(6.716938, 110.0)).abs() < 1e-9);
}

#[test]
fn the_natural_length_is_measured_from_the_in_point() {
    // webClipLength.test.ts: is measured FROM the in-point (615).
    let mut m = table("/a/r.wav", 10.0);
    let w = drawn(json!({"source": "/a/r.wav", "sourceFrom": 2}), Some(110.0), &mut m);
    assert!((w.length - beats(8.0, 110.0)).abs() < 1e-9);
}

#[test]
fn a_file_that_is_not_there_or_no_tempo_or_a_midi_clip_stays_zero() {
    // webClipLength.test.ts: stays 0 when the file is not there / with no tempo / leaves MIDI alone (615).
    let mut m = Counting::default();
    assert_eq!(
        drawn(json!({"source": "C:/nowhere/gone.wav"}), Some(110.0), &mut m).length,
        0.0
    );
    let mut m = table("/a/r.wav", 4.0);
    assert_eq!(drawn(json!({"source": "/a/r.wav"}), None, &mut m).length, 0.0);
    assert_eq!(
        drawn(json!({"type": "midi", "source": ""}), Some(110.0), &mut m).length,
        0.0
    );
    assert_eq!(drawn(json!({"source": "/a/r.wav"}), Some(0.0), &mut m).length, 0.0);
    // The in-point past the end leaves nothing to draw.
    assert_eq!(
        drawn(json!({"source": "/a/r.wav", "sourceFrom": 9}), Some(110.0), &mut m).length,
        0.0
    );
}

#[test]
fn a_stated_extent_wins_and_the_file_is_not_asked() {
    // webClipLength.test.ts: keeps it — a window is the document's business (615).
    let mut m = table("/a/r.wav", 10.0);
    assert_eq!(
        drawn(
            json!({"source": "/a/r.wav", "length": 4, "sourceTo": 2.18}),
            Some(110.0),
            &mut m
        )
        .length,
        4.0
    );
    assert_eq!(m.asked, 0);
}

#[test]
fn the_window_into_the_material_as_the_document_states_it() {
    // webTracks.test.ts: the WINDOW into its material; an ABSENT window stays absent (615).
    let w = drawn(
        json!({"offset": 0.5, "sourceFrom": 1.25, "sourceTo": 3}),
        None,
        &mut Counting::default(),
    );
    assert_eq!(
        (w.offset, w.source_from, w.source_to),
        (Some(0.5), Some(1.25), Some(3.0))
    );
    let w = drawn(json!({}), None, &mut Counting::default());
    assert_eq!((w.source_from, w.source_to), (None, None));
}

#[test]
fn the_source_name_and_never_its_path() {
    // webTracks.test.ts: the source's NAME, never its path; no name for a MIDI clip (615).
    let w = drawn(
        json!({"source": "/srv/root/f/ws/song/clips/riff_warm.wav"}),
        None,
        &mut Counting::default(),
    );
    assert_eq!(w.source_name.as_deref(), Some("riff_warm.wav"));
    assert!(!json_of(&w).to_string().contains("/srv/"));
    assert_eq!(
        drawn(json!({"type": "midi"}), None, &mut Counting::default()).source_name,
        None
    );
    // A `\` is a character of a name here: the source is a spelled `/` path.
    let w = drawn(json!({"source": "a\\b.wav"}), None, &mut Counting::default());
    assert_eq!(w.source_name.as_deref(), Some("a\\b.wav"));
}

#[test]
fn fades_are_carried_and_a_zero_fade_is_absent() {
    // webTracks.test.ts: carries the FADES / a zero fade is ABSENT (615).
    let w = drawn(json!({"fadeIn": 1, "fadeOut": 2.5}), None, &mut Counting::default());
    assert_eq!((w.fade_in, w.fade_out), (Some(1.0), Some(2.5)));
    let w = drawn(json!({}), None, &mut Counting::default());
    assert_eq!((w.fade_in, w.fade_out), (None, None));
}

#[test]
fn warped_disabled_frozen_and_the_notes_follow_to_web_clip() {
    // decision 615: `warped` is a map or a declined map, the flags are true or absent, vel 1 and repeat 0 are absent.
    let w = drawn(
        json!({"warp": {"algorithm": "a", "markers": []}, "disabled": true, "id": "x__frozen"}),
        None,
        &mut Counting::default(),
    );
    assert_eq!(
        (w.warped, w.warp_off, w.disabled, w.frozen),
        (Some(true), None, Some(true), Some(true))
    );
    let w = drawn(
        json!({"warpOff": true, "disabled": false}),
        None,
        &mut Counting::default(),
    );
    assert_eq!(
        (w.warped, w.warp_off, w.disabled, w.frozen),
        (Some(true), Some(true), None, None)
    );
    let notes = json!([
        {"time": 0, "duration": 1, "key": 60, "vel": 1, "channel": 0, "rel": 0},
        {"time": 1, "duration": 1, "key": 61, "vel": 0.5, "channel": 0, "rel": 0, "repeat": 3, "skip": 0}
    ]);
    let v = json_of(&drawn(
        json!({"type": "midi", "notes": notes}),
        None,
        &mut Counting::default(),
    ));
    assert_eq!(v["notes"][0], json!({"time": 0.0, "duration": 1.0, "key": 60}));
    assert_eq!(
        v["notes"][1],
        json!({"time": 1.0, "duration": 1.0, "key": 61, "vel": 0.5, "repeat": 3})
    );
}

fn web(over: Value) -> WebClip {
    drawn(over, None, &mut Counting::default())
}

fn piece(id: &str, of: &str, start: f64, length: f64, extra: Value) -> WebClip {
    let mut c = json!({"id": id, "name": id, "start": start, "length": length});
    c.as_object_mut()
        .expect("object")
        .extend(extra.as_object().cloned().unwrap_or_default());
    let mut w = web(c);
    w.comp = Some(song::clips::CompMark {
        of: of.to_owned(),
        take: id.to_owned(),
    });
    w
}

#[test]
fn a_comped_event_folds_into_one_clip_and_the_rest_stay() {
    // webComp.test.ts: folds a comped event's pieces into one clip, spanning them, and leaves the rest (616).
    let folded = fold_comps(vec![
        web(json!({"id": "x@0", "name": "x", "start": 0, "length": 2})),
        piece("gtr@4", "gtr@4", 4.0, 2.0, json!({"fadeIn": 0.5})),
        piece("t2@6", "gtr@4", 6.0, 6.0, json!({})),
    ]);
    assert_eq!(folded.len(), 2);
    let f = &folded[1];
    assert_eq!(
        (f.id.as_str(), f.name.as_str(), f.start, f.length, f.fade_in),
        ("gtr@4", "gtr", 4.0, 8.0, Some(0.5))
    );
    assert_eq!(
        f.comp.as_ref().map(|c| (c.of.as_str(), c.take.as_str())),
        Some(("gtr@4", ""))
    );
    let ids: Vec<_> = f
        .pieces
        .as_ref()
        .expect("pieces")
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, ["gtr@4", "t2@6"]);
    assert!(f.notes.is_empty() && f.source_name.is_none() && f.offset.is_none());
}

#[test]
fn pieces_are_sorted_stably_and_the_start_is_the_events_own() {
    // decision 616: pieces out of order sort by start; `start` is the text after the last `@`, else the first piece's.
    let f = &fold_comps(vec![
        piece("b", "ev@3", 7.0, 1.0, json!({"disabled": true})),
        piece("a", "ev@3", 5.0, 1.0, json!({})),
    ])[0];
    assert_eq!((f.start, f.length, f.disabled), (3.0, 5.0, Some(true)));
    let ids: Vec<_> = f
        .pieces
        .as_ref()
        .expect("pieces")
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b"]);
    // No number after the `@` falls back to the first piece, and an empty one is 0.
    assert_eq!(fold_comps(vec![piece("a", "ev@x", 5.0, 1.0, json!({}))])[0].start, 5.0);
    assert_eq!(fold_comps(vec![piece("a", "ev@", 5.0, 1.0, json!({}))])[0].start, 0.0);
    let no_at = &fold_comps(vec![piece("a", "ev", 5.0, 1.0, json!({}))])[0];
    assert_eq!((no_at.name.as_str(), no_at.start), ("e", 5.0));
}

#[test]
fn the_last_piece_is_the_first_whose_end_is_the_greatest() {
    // decision 616 and the field tables: foldComps's last.
    let f = &fold_comps(vec![
        piece("a", "e@0", 0.0, 4.0, json!({"fadeOut": 1})),
        piece("b", "e@0", 2.0, 2.0, json!({"fadeOut": 3})),
    ])[0];
    assert_eq!(f.fade_out, Some(1.0));
}

fn nest(of: &str, rf: &str, start: f64, length: f64, event: Option<Value>) -> Value {
    let mut n = json!({"of": of, "ref": rf, "start": start, "length": length});
    if let Some(e) = event {
        n["event"] = e;
    }
    n
}

fn fold(clips: Vec<Value>) -> Vec<WebClip> {
    let docs: Vec<_> = clips.into_iter().map(clip).collect();
    let mut m = Counting::default();
    fold_nests(&docs, &mut |c| web_clip(c, None, &mut m)).expect("reads")
}

#[test]
fn a_timeline_event_folds_its_clips_into_one_at_the_events_start() {
    // webNest.test.ts: folds its clips into one, at the event's start and the timeline's length (617).
    let n = nest("verse@8", "verse", 8.0, 8.0, None);
    let t = fold(vec![
        json!({"id": "a@0", "length": 2}),
        json!({"id": "verse@8>a@0", "start": 8, "length": 2, "nest": n}),
        json!({"id": "verse@8>b@4", "name": "b", "type": "midi", "start": 12, "length": 2, "nest": n}),
    ]);
    let got: Vec<_> = t.iter().map(|c| (c.id.as_str(), c.start, c.length)).collect();
    assert_eq!(got, [("a@0", 0.0, 2.0), ("verse@8", 8.0, 8.0)]);
    let tl = t[1].timeline.as_ref().expect("a timeline");
    assert_eq!(tl.ref_, "verse");
    assert_eq!(
        tl.clips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["verse@8>a@0", "verse@8>b@4"]
    );
    assert_eq!(json_of(&t[1])["event"], json!({}));
}

#[test]
fn a_timeline_is_midi_only_when_everything_it_holds_is() {
    // webNest.test.ts: is MIDI only when everything it holds is (617).
    let n = nest("verse@8", "verse", 8.0, 8.0, None);
    let t = fold(vec![
        json!({"id": "verse@8>b@0", "type": "midi", "start": 8, "nest": n}),
    ]);
    assert_eq!(json_of(&t[0])["type"], "midi");
}

#[test]
fn a_timeline_is_drawn_muted_when_the_event_is() {
    // webNest.test.ts: is drawn muted when the event is (617).
    let n = nest("verse@8", "verse", 8.0, 8.0, Some(json!({"disabled": true})));
    let t = fold(vec![
        json!({"id": "verse@8>a@0", "start": 8, "disabled": true, "nest": n}),
    ]);
    assert_eq!(t[0].disabled, Some(true));
    assert_eq!(json_of(&t[0])["event"], json!({"disabled": true}));
}

#[test]
fn a_comped_event_inside_a_nest_is_one_clip_inside_it() {
    // decision 617: the inner clips are folded before the lane is.
    let n = nest("v@0", "v", 0.0, 8.0, None);
    let comp = |id: &str, start: f64| json!({"id": id, "start": start, "length": 1, "nest": n, "comp": {"of": "v@0>g@0", "take": id}});
    let t = fold(vec![comp("v@0>g@0", 0.0), comp("v@0>t@2", 2.0)]);
    let inner = &t[0].timeline.as_ref().expect("timeline").clips;
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].pieces.as_ref().map(Vec::len), Some(2));
}

#[test]
fn the_folds_are_linear_and_their_worst_cases_hold() {
    // hostile table (fold_comps, fold_nests): 100,000 pieces of one comp, 100,000 comps of one piece, huge start.
    let base = piece("p", "e@0", 0.0, 1.0, json!({}));
    let pieces: Vec<WebClip> = (0..100_000)
        .map(|i| WebClip {
            id: format!("p{i}"),
            start: f64::from(i),
            ..base.clone()
        })
        .collect();
    let one = fast("one comp of 100,000 pieces", || fold_comps(pieces));
    assert_eq!((one.len(), one[0].pieces.as_ref().map(Vec::len)), (1, Some(100_000)));
    let comps: Vec<WebClip> = (0..100_000)
        .map(|i| WebClip {
            comp: Some(song::clips::CompMark {
                of: format!("e{i}@0"),
                take: "t".into(),
            }),
            ..base.clone()
        })
        .collect();
    let many = fast("100,000 comps of one piece", || fold_comps(comps));
    assert_eq!(many.len(), 100_000);
    let big = fold_comps(vec![piece("a", "e@1e300", 5.0, 1.0, json!({}))]);
    assert_eq!(big[0].start, 1e300);
    let one_clip = clip(json!({"id": "n", "length": 1}));
    let docs: Vec<_> = (0..100_000)
        .map(|i| {
            let mut c = one_clip.clone();
            c.nest = Some(song::clips::NestDoc {
                of: format!("n{i}"),
                ref_: "n".into(),
                start: 0.0,
                length: 1.0,
                event: None,
            });
            c
        })
        .collect();
    let folded = |docs: &[song::clips::ClipDoc]| {
        let mut m = Counting::default();
        fold_nests(docs, &mut |c| web_clip(c, None, &mut m)).expect("reads")
    };
    assert_eq!(fast("100,000 nests of 1 clip", || folded(&docs)).len(), 100_000);
    let same: Vec<_> = docs
        .iter()
        .map(|c| {
            let mut c = c.clone();
            c.nest = Some(song::clips::NestDoc {
                of: "v@0".into(),
                ref_: "v".into(),
                start: 0.0,
                length: 8.0,
                event: None,
            });
            c.comp = Some(song::clips::CompMark {
                of: "v@0>g@0".into(),
                take: "t".into(),
            });
            c
        })
        .collect();
    let comped = fast("a nest of 100,000 comped clips", || folded(&same));
    assert_eq!(comped[0].timeline.as_ref().map(|t| t.clips.len()), Some(1));
}

#[test]
fn a_hundred_thousand_clips_over_one_source_measure_it_once() {
    // hostile table (web_clips): 100,000 clips on a counting Measure over one source; 100,000 notes on one clip.
    let docs: Vec<_> = (0..100_000).map(|_| clip(json!({"source": "/a/r.wav"}))).collect();
    let mut m = table("/a/r.wav", 2.0);
    let out = fast("100,000 clips", || {
        web_clips(&docs, Some(120.0), &mut m).expect("reads")
    });
    assert_eq!(out.len(), 100_000);
    assert_eq!(m.asked, 100_000);
    let notes: Vec<Value> = (0..100_000)
        .map(|i| json!({"time": i, "duration": 1, "key": 60, "vel": 0.5, "channel": 0, "rel": 0}))
        .collect();
    let w = fast("100,000 notes", || web(json!({"type": "midi", "notes": notes})));
    assert_eq!(w.notes.len(), 100_000);
}

#[test]
fn a_length_past_the_double_range_is_the_largest_double_and_not_infinity() {
    // The output rule forbids `null`: tempo 1e308 over 100 s would be `inf` (615).
    let mut m = table("/a/r.wav", 100.0);
    let w = drawn(json!({"source": "/a/r.wav"}), Some(1e308), &mut m);
    assert_eq!(w.length, f64::MAX);
    assert!(json_of(&w)["length"].is_number());
}
