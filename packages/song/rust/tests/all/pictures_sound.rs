// pictures_sound.rs — a shot's sound where the picture shows it: the eight cases of videoSound.test.ts and the decisions of the pieces.
// A reader would get wrong the bps: 120 bpm is two beats a second, which every expected number below is read in.

use crate::clips_support;
use crate::pictures_support;
use crate::support;

use clips_support::clip_family;
use pictures_support::{Counting, Failing, own, pieces_with, placements};
use serde_json::{Value, json};
use song::pictures::{SoundPiece, Spend};
use song::{Severity, SourceError};

type Row = (String, f64, f64, f64, Option<f64>, f64, f64);

fn clips() -> song::FamilyRead<song::Clip> {
    clip_family(&[
        ("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"})),
        ("b", json!({"source": "clips/b.mp4", "sound": "clips/b.wav"})),
        ("mute", json!({"source": "clips/mute.mp4"})),
    ])
}

fn disk() -> song::MemSource {
    support::mem(&[("clips/a.wav", ""), ("clips/b.wav", "")])
}

/// The sound of `events` at 120 bpm, in the shape the TypeScript test reads.
fn sound(events: &[Value]) -> Vec<Row> {
    let evs = placements(events);
    let out = made(&evs, 2.0).0;
    out.iter()
        .map(|p| {
            let c = &p.clip;
            (
                c.name.clone(),
                c.start,
                c.length,
                c.source_from.unwrap_or(0.0),
                c.source_to,
                c.fade_in,
                c.fade_out,
            )
        })
        .collect()
}

fn made(evs: &[song::Placement], bps: f64) -> (Vec<SoundPiece>, Spend) {
    let src = disk();
    let mut spend = Spend::new();
    let shots = own(evs);
    let out = pieces_with(&src, &clips(), bps, &shots, &mut spend).expect("pieces");
    (out, spend)
}

fn row(name: &str, start: f64, length: f64, from: f64, to: Option<f64>, fade_in: f64, fade_out: f64) -> Row {
    (name.to_owned(), start, length, from, to, fade_in, fade_out)
}

#[test]
fn a_shot_is_placed_in_beats_its_trim_the_window_into_the_sound_file() {
    // videoSound.test.ts "places a shot in beats", decisions 509 and 508
    assert_eq!(
        sound(&[json!({"ref": "a", "start": 1, "length": 2, "from": 0.5})]),
        [row("a", 2.0, 4.0, 0.5, Some(2.5), 0.0, 0.0)]
    );
}

#[test]
fn a_silent_shot_plays_nothing_and_still_covers_the_one_under_it() {
    // videoSound.test.ts "plays nothing for a silent shot", decision 511
    let got = sound(&[
        json!({"ref": "a", "start": 0, "length": 10}),
        json!({"ref": "b", "start": 2, "length": 3, "silent": true}),
    ]);
    assert_eq!(
        got.iter().map(|r| (r.1, r.2)).collect::<Vec<_>>(),
        [(0.0, 4.0), (10.0, 10.0)]
    );
}

#[test]
fn a_later_shot_cuts_the_earlier_sound_and_it_comes_back_after() {
    // videoSound.test.ts "cuts the earlier sound where a later shot covers it", decision 506
    assert_eq!(
        sound(&[
            json!({"ref": "a", "start": 0, "length": 10}),
            json!({"ref": "b", "start": 2, "length": 3})
        ]),
        [
            row("a", 0.0, 4.0, 0.0, Some(2.0), 0.0, 0.0),
            row("a", 10.0, 10.0, 5.0, Some(10.0), 0.0, 0.0),
            row("b", 4.0, 6.0, 0.0, Some(3.0), 0.0, 0.0),
        ]
    );
}

#[test]
fn the_two_sounds_cross_over_the_incoming_shots_fade_in() {
    // videoSound.test.ts "crosses the two sounds", decision 507
    assert_eq!(
        sound(&[
            json!({"ref": "a", "start": 0, "length": 5}),
            json!({"ref": "b", "start": 4, "length": 5, "fadeIn": {"over": 1, "with": "wipe"}}),
        ]),
        [
            row("a", 0.0, 10.0, 0.0, Some(5.0), 0.0, 2.0),
            row("b", 8.0, 10.0, 0.0, Some(5.0), 2.0, 0.0)
        ]
    );
}

#[test]
fn a_length_zero_shot_runs_to_the_next_start_and_the_last_to_the_end_of_its_file() {
    // videoSound.test.ts "runs a length-0 shot to the next start", decision 504
    assert_eq!(
        sound(&[
            json!({"ref": "a", "start": 0, "length": 0}),
            json!({"ref": "b", "start": 3, "length": 0})
        ]),
        [
            row("a", 0.0, 6.0, 0.0, Some(3.0), 0.0, 0.0),
            row("b", 6.0, 0.0, 0.0, None, 0.0, 0.0)
        ]
    );
}

#[test]
fn a_split_shot_sounds_as_one_the_tail_picking_up_where_the_head_left_off() {
    // videoSound.test.ts "plays a SPLIT shot as one", decision 509
    assert_eq!(
        sound(&[
            json!({"ref": "a", "start": 0, "length": 1.5, "from": 0.5}),
            json!({"ref": "a", "start": 1.5, "length": 2.5, "from": 2}),
        ]),
        [
            row("a", 0.0, 3.0, 0.5, Some(2.0), 0.0, 0.0),
            row("a", 3.0, 5.0, 2.0, Some(4.5), 0.0, 0.0)
        ]
    );
}

#[test]
fn a_speed_retimes_the_sound_and_a_freeze_and_a_reverse_are_silent() {
    // videoSound.test.ts "retimes a shot's sound by its speed", decisions 509 and 511
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 2, "from": 1, "speed": 2}),
        json!({"ref": "b", "start": 2, "length": 2, "speed": 0}),
        json!({"ref": "a", "start": 4, "length": 2, "from": 5, "speed": -1}),
    ]);
    let (out, _) = made(&evs, 2.0);
    assert_eq!(out.len(), 1);
    let c = &out[0].clip;
    assert_eq!(
        (c.start, c.length, c.source_from, c.source_to),
        (0.0, 4.0, Some(1.0), Some(5.0))
    );
    let warp = c.warp.as_ref().expect("a warp");
    assert_eq!(
        (warp.algorithm.as_str(), warp.markers.clone()),
        ("varispeed", vec![(0.0, 1.0), (2.0, 3.0)])
    );
}

#[test]
fn a_shot_keeps_its_fades_and_a_clip_with_no_sound_and_an_activation_are_silent() {
    // videoSound.test.ts "keeps a shot's own fades, and is silent", decisions 511 and 504
    assert_eq!(
        sound(&[
            json!({"ref": "a", "start": 0, "length": 4, "fadeIn": 1, "fadeOut": 0.5}),
            json!({"ref": "mute", "start": 4, "length": 2}),
            json!({"start": 6, "length": 2}),
        ]),
        [row("a", 0.0, 8.0, 0.0, Some(4.0), 2.0, 1.0)]
    );
}

#[test]
fn a_piece_has_the_id_the_source_the_name_and_the_fields_the_engine_reads() {
    // decision 509: field for field, a pitch kept is elastique, notes present as []
    let evs = placements(&[json!({"ref": "a", "start": 1.5, "length": 2, "from": 1, "speed": 0.5, "pitch": "keep"})]);
    let (out, _) = made(&evs, 2.0);
    let c = &out[0].clip;
    assert_eq!(c.id, "a@3");
    assert_eq!(c.source, "/p/clips/a.wav");
    assert_eq!((c.offset, c.loop_start, c.loop_end), (0.0, 0.0, 0.0));
    assert!(c.notes.is_empty());
    assert_eq!(c.warp.as_ref().map(|w| w.algorithm.as_str()), Some("elastique"));
    assert_eq!(c.source_to, Some(2.0));
    let v = serde_json::to_value(c).expect("json");
    assert!(v.get("notes").is_some() && v.get("gainDb").is_none() && v.get("sourceFrom").is_some());
}

#[test]
fn a_shot_ends_at_the_next_event_even_an_activation_and_keeps_its_index() {
    // decision 504: the end is the NEXT EVENT's start, an activation included, and the piece keeps its place in the list
    let evs = placements(&[
        json!({"ref": "a", "start": 0}),
        json!({"start": 3}),
        json!({"ref": "b", "start": 3, "length": 1}),
    ]);
    let (out, _) = made(&evs, 2.0);
    assert_eq!(
        out.iter()
            .map(|p| (p.shot, p.clip.name.as_str(), p.clip.length))
            .collect::<Vec<_>>(),
        [(0, "a", 6.0), (2, "b", 2.0)]
    );
}

#[test]
fn a_bare_number_string_is_its_number_and_a_colon_makes_a_shot_without_sound() {
    // decisions 502 and 505: "2.5" is 2.5; "2:1" is NaN, so it sounds nothing and covers nothing
    let (out, _) = made(&placements(&[json!({"ref": "a", "start": "2.5", "length": 1})]), 2.0);
    assert_eq!((out[0].clip.start, out[0].clip.length), (5.0, 2.0));
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 10}),
        json!({"ref": "b", "start": "2:1", "length": 3}),
    ]);
    let (out, _) = made(&evs, 2.0);
    assert_eq!(
        out.iter()
            .map(|p| (p.clip.name.as_str(), p.clip.start, p.clip.length))
            .collect::<Vec<_>>(),
        [("a", 0.0, 20.0)]
    );
}

#[test]
fn of_two_later_shots_with_one_from_the_last_in_the_file_gives_the_fade_out() {
    // decision 507: both fades end at 4; the second in the file, with two seconds, wins over the first's one
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 9}),
        json!({"ref": "b", "start": 3, "length": 6, "fadeIn": 1}),
        json!({"ref": "b", "start": 2, "length": 7, "fadeIn": 2}),
    ]);
    let (out, _) = made(&evs, 1.0);
    let a = out.iter().find(|p| p.clip.name == "a").expect("a");
    assert_eq!((a.clip.length, a.clip.fade_out), (4.0, 2.0));
}

#[test]
fn a_cross_wins_over_the_shots_own_fade_out() {
    // decision 507 and the order of the fade: crossAt first, the shot's own only at its end
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 5, "fadeOut": 2}),
        json!({"ref": "b", "start": 4, "length": 5, "fadeIn": 1}),
    ]);
    let (out, _) = made(&evs, 1.0);
    assert_eq!(out[0].clip.fade_out, 1.0);
    let evs = placements(&[json!({"ref": "a", "start": 0, "length": 5, "fadeOut": 2})]);
    assert_eq!(made(&evs, 1.0).0[0].clip.fade_out, 2.0);
}

#[test]
fn a_fade_clamped_to_its_shot_and_a_fade_on_an_open_shot() {
    // decisions 503 and 504: a fade is at most the span, an open shot has no fade out
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 1, "fadeIn": 5, "fadeOut": 5}),
        json!({"ref": "b", "start": 4, "fadeIn": 3, "fadeOut": 3}),
    ]);
    let (out, _) = made(&evs, 1.0);
    assert_eq!((out[0].clip.fade_in, out[0].clip.fade_out), (1.0, 1.0));
    assert_eq!(
        (out[1].clip.fade_in, out[1].clip.fade_out, out[1].clip.length),
        (3.0, 0.0, 0.0)
    );
}

#[test]
fn a_timecode_fade_is_no_fade() {
    // decision 503: a string has no `over`
    let evs = placements(&[json!({"ref": "a", "start": 0, "length": 2, "fadeIn": "1:0"})]);
    assert_eq!(made(&evs, 1.0).0[0].clip.fade_in, 0.0);
}

#[test]
fn a_speed_not_finite_is_one_and_a_pitch_of_tape_is_varispeed() {
    // decisions 504 and 509
    let mut evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 1}),
        json!({"ref": "b", "start": 2, "length": 1, "speed": 0.5, "pitch": "tape"}),
    ]);
    evs[0].speed = Some(f64::INFINITY);
    let (out, _) = made(&evs, 1.0);
    assert!(out[0].clip.warp.is_none());
    assert_eq!(
        out[1].clip.warp.as_ref().map(|w| w.algorithm.as_str()),
        Some("varispeed")
    );
}

#[test]
fn only_a_video_clip_with_a_sound_word_sounds() {
    // decision 511: an audio clip, a caption, an empty word and a ref that is absent sound nothing
    let clips = clip_family(&[
        ("aud", json!({"source": "clips/x.wav"})),
        ("empty", json!({"source": "clips/e.mp4", "sound": ""})),
        ("a", json!({"source": "clips/a.mp4", "sound": "clips/a.wav"})),
    ]);
    let evs = placements(&[
        json!({"ref": "aud", "start": 0, "length": 1}),
        json!({"ref": "empty", "start": 1, "length": 1}),
        json!({"ref": "nope", "start": 2, "length": 1}),
        json!({"ref": "a", "start": 3, "length": 1}),
    ]);
    let src = disk();
    let mut spend = Spend::new();
    let out = pieces_with(&src, &clips, 1.0, &own(&evs), &mut spend).expect("pieces");
    assert_eq!(out.iter().map(|p| p.shot).collect::<Vec<_>>(), [3]);
}

#[test]
fn one_stat_a_word_and_none_for_a_sound_fully_covered() {
    // decision 512: stat once per distinct word, only when a piece is about to be made
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 1}),
        json!({"ref": "a", "start": 2, "length": 1}),
        json!({"ref": "b", "start": 0, "length": 3}),
    ]);
    let src = disk();
    let counting = Counting::new(&src);
    let mut spend = Spend::new();
    pieces_with(&counting, &clips(), 1.0, &own(&evs), &mut spend).expect("pieces");
    assert_eq!(counting.stats(), ["/p/clips/b.wav"]);
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 1}),
        json!({"ref": "a", "start": 2, "length": 1}),
        json!({"ref": "a", "start": 4, "length": 1}),
    ]);
    let counting = Counting::new(&src);
    pieces_with(&counting, &clips(), 1.0, &own(&evs), &mut Spend::new()).expect("pieces");
    assert_eq!(counting.stats().len(), 1);
}

#[test]
fn a_sound_not_on_disk_keeps_its_path_and_a_folder_counts_as_there() {
    // decision 512: NotFound keeps the path (the fault is the lanes' to make), Ok of any kind counts
    let src = support::mem(&[("clips/a.wav/inside", "")]);
    let evs = placements(&[
        json!({"ref": "a", "start": 0, "length": 1}),
        json!({"ref": "b", "start": 1, "length": 1}),
    ]);
    let out = pieces_with(&src, &clips(), 1.0, &own(&evs), &mut Spend::new()).expect("pieces");
    assert_eq!(
        out.iter().map(|p| p.clip.source.as_str()).collect::<Vec<_>>(),
        ["/p/clips/a.wav", "/p/clips/b.wav"]
    );
}

#[test]
fn an_absolute_word_and_a_word_the_source_refuses() {
    // decisions 512 and 514: Outside, a NUL and an Io give no pieces and one warning on the clip's file per word
    let clips = clip_family(&[
        ("up", json!({"source": "a.mp4", "sound": "../../x.wav"})),
        ("abs", json!({"source": "b.mp4", "sound": "/etc/passwd"})),
        ("nul", json!({"source": "c.mp4", "sound": "clips/a\u{0}b.wav"})),
        ("ok", json!({"source": "d.mp4", "sound": "clips/a.wav"})),
    ]);
    let evs = placements(&[
        json!({"ref": "up", "start": 0, "length": 1}),
        json!({"ref": "up", "start": 1, "length": 1}),
        json!({"ref": "abs", "start": 2, "length": 1}),
        json!({"ref": "nul", "start": 3, "length": 1}),
        json!({"ref": "ok", "start": 4, "length": 1}),
    ]);
    let src = disk();
    let mut spend = Spend::new();
    let out = pieces_with(&src, &clips, 1.0, &own(&evs), &mut spend).expect("pieces");
    assert_eq!(out.iter().map(|p| p.shot).collect::<Vec<_>>(), [4]);
    let mut files: Vec<(&str, Severity)> = spend.findings().iter().map(|f| (f.file.as_str(), f.severity)).collect();
    files.sort_by_key(|f| f.0);
    assert_eq!(
        files,
        [
            ("clips/abs.k7x", Severity::Warning),
            ("clips/nul.k7x", Severity::Warning),
            ("clips/up.k7x", Severity::Warning)
        ]
    );
}

#[test]
fn a_stat_that_fails_is_a_warning_and_a_stat_the_bundle_lacks_is_an_error_to_the_caller() {
    // decision 514: Io costs that sound; Missing is Err
    let evs = placements(&[json!({"ref": "a", "start": 0, "length": 1})]);
    let mut spend = Spend::new();
    let out = pieces_with(
        &Failing(SourceError::Io("boom".into())),
        &clips(),
        1.0,
        &own(&evs),
        &mut spend,
    )
    .expect("pieces");
    assert!(out.is_empty());
    assert_eq!(spend.findings().len(), 1);
    let again = pieces_with(
        &Failing(SourceError::Missing),
        &clips(),
        1.0,
        &own(&evs),
        &mut Spend::new(),
    );
    assert!(matches!(again, Err(SourceError::Missing)));
}

#[test]
fn a_number_that_is_not_finite_drops_the_piece_with_one_error_per_clip_file() {
    // decision 510: `start` near 1e308 at 300 bpm overflows the beat
    let evs = placements(&[
        json!({"ref": "a", "start": 1e308, "length": 1e308}),
        json!({"ref": "a", "start": 1e308, "length": 1e307, "speed": 1e300}),
        json!({"ref": "b", "start": 0, "length": 1}),
    ]);
    let src = disk();
    let mut spend = Spend::new();
    let out = pieces_with(&src, &clips(), 5.0, &own(&evs), &mut spend).expect("pieces");
    assert_eq!(out.iter().map(|p| p.clip.name.as_str()).collect::<Vec<_>>(), ["b"]);
    let errors: Vec<&str> = spend
        .findings()
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.as_str())
        .collect();
    assert_eq!(errors, ["clips/a.k7x"]);
    assert!(
        serde_json::to_string(&out[0].clip)
            .expect("json")
            .find("null")
            .is_none()
    );
}

#[test]
fn a_negative_zero_start_is_zero() {
    // decisions 502 and 506: `-0` is a plain 0 in a key and in an id
    let evs = placements(&[json!({"ref": "a", "start": -0.0, "length": 1})]);
    let c = &made(&evs, 1.0).0[0].clip;
    assert_eq!(c.id, "a@0");
}
