// decode.rs — the ported TypeScript cases: the notes short form, a finding's position, the entity schemas' refusals.
// A reader would get wrong that wording is not pinned: a case that fixes a message is left out, only that it refuses is held.

use crate::support;

use k7x::{Bump, parse_tree};
use song::files::{expand_notes, line_col, locate};
use song::{Cache, Clip, Finding, Sequence, Timeline, read_family};
use support::mem;

fn n(t: f64, d: f64, k: u8, v: Option<f64>) -> (f64, f64, u8, Option<f64>) {
    (t, d, k, v)
}

fn laid(src: &str, step: f64, v: Option<f64>) -> Vec<(f64, f64, u8, Option<f64>)> {
    expand_notes(src, step, v)
        .expect("reads")
        .iter()
        .map(|x| (x.t.0, x.d.0, x.k.0, x.v.map(|v| v.0)))
        .collect()
}

fn one<T: song::FileValue>(file: &str, text: &str) -> Result<std::sync::Arc<T>, Finding> {
    let src = mem(&[(file, text)]);
    let fam = read_family::<T>(&src, &mut Cache::new(), "/p").expect("reads");
    match fam.values.into_iter().next() {
        Some((_, v)) => Ok(v),
        None => Err(fam.findings.into_iter().next().expect("a finding")),
    }
}

#[test]
fn lays_one_step_a_beat_a_rest_leaving_its_beat_empty() {
    // sugar.test.ts: lays one step a beat, a rest leaving its beat empty.
    assert_eq!(
        laid("c4 d4 ~ e4", 1.0, None),
        [n(0.0, 1.0, 60, None), n(1.0, 1.0, 62, None), n(3.0, 1.0, 64, None)]
    );
}

#[test]
fn splits_a_step_with_brackets_and_stacks_a_chord_with_a_comma() {
    // sugar.test.ts: splits a step with [ ], stacks a chord with a comma.
    assert_eq!(
        laid("[c4 d4] e4", 1.0, None),
        [n(0.0, 0.5, 60, None), n(0.5, 0.5, 62, None), n(1.0, 1.0, 64, None)]
    );
    assert_eq!(
        laid("[c4,e4,g4] ~", 1.0, None),
        [n(0.0, 1.0, 60, None), n(0.0, 1.0, 64, None), n(0.0, 1.0, 67, None)]
    );
}

#[test]
fn lengthens_a_step_with_at_and_repeats_it_with_bang() {
    // sugar.test.ts: lengthens a step with @ and repeats it with !.
    assert_eq!(
        laid("c4@2 d4", 1.0, None),
        [n(0.0, 2.0, 60, None), n(2.0, 1.0, 62, None)]
    );
    assert_eq!(
        laid("c4!3", 1.0, None),
        [n(0.0, 1.0, 60, None), n(1.0, 1.0, 60, None), n(2.0, 1.0, 60, None)]
    );
}

#[test]
fn takes_a_step_length_a_velocity_sharps_flats_and_midi_numbers() {
    // sugar.test.ts: takes a step length and a velocity, sharps, flats and MIDI numbers.
    let v = Some(0.8);
    assert_eq!(
        laid("c#4 db4 64", 0.5, v),
        [n(0.0, 0.5, 61, v), n(0.5, 0.5, 61, v), n(1.0, 0.5, 64, v)]
    );
    assert_eq!(laid("a-1", 1.0, None), [n(0.0, 1.0, 9, None)]);
}

#[test]
fn refuses_what_does_not_read() {
    // sugar.test.ts: names what does not read; only the refusal is held, not its wording.
    assert!(expand_notes("c4 h4", 1.0, None).is_err());
    assert!(expand_notes("[c4 d4", 1.0, None).is_err());
    assert!(expand_notes("c4 300", 1.0, None).is_err());
    assert!(expand_notes("c4 ]", 1.0, None).is_err());
}

#[test]
fn a_short_form_passes_and_a_note_that_does_not_read_is_placed_on_its_line() {
    // sugar.test.ts: passes its schema, and a note that does not read is placed on its line.
    let ok = one::<Sequence>("sequences/m.k7x", "notes(\"c4 d4 e4\")\nto(\"2:1\")\n").expect("reads");
    assert_eq!(ok.notes.0.len(), 3);
    let bad = one::<Sequence>("sequences/m.k7x", "// a riff\nnotes(\"c4 x9\")\n").expect_err("refused");
    assert_eq!((bad.line, bad.col), (2, 7));
}

#[test]
fn the_seq_form_takes_step_and_velocity_and_refuses_the_rest() {
    let src = "notes(seq(\"c4 d4\").step(0.5).v(0.8))\n";
    let seq = one::<Sequence>("sequences/m.k7x", src).expect("reads");
    assert_eq!((seq.notes.0[1].t.0, seq.notes.0[1].v.map(|v| v.0)), (0.5, Some(0.8)));
    assert!(one::<Sequence>("sequences/m.k7x", "notes(seq(\"c4\").loud(1))\n").is_err());
    assert!(one::<Sequence>("sequences/m.k7x", "notes(seq(\"c4\").step(0))\n").is_err());
    assert!(one::<Sequence>("sequences/m.k7x", "notes(seq(\"c4\").v(2))\n").is_err());
}

#[test]
fn a_list_of_notes_reads_as_written() {
    let text = "from(\"1:1\")\nnotes([\n  t(0).d(1).k(60),\n  t(1).d(0.5).k(64).v(0.4),\n])\n";
    let seq = one::<Sequence>("sequences/m.k7x", text).expect("reads");
    assert_eq!(seq.notes.0.len(), 2);
    assert!(one::<Sequence>("sequences/m.k7x", "notes([t(0).d(1).k(200)])\n").is_err());
    assert!(one::<Sequence>("sequences/m.k7x", "notes([t(0).d(0).k(60)])\n").is_err());
    assert!(one::<Sequence>("sequences/m.k7x", "notes([t(0).d(1).k(60).repeat(1)])\n").is_err());
}

fn pos(text: &str, path: &[&str]) -> Option<(u32, u32)> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).ok()?;
    let path: Vec<k7x::PathSeg> = path
        .iter()
        .map(|s| {
            s.parse::<usize>()
                .map_or_else(|_| k7x::PathSeg::Key((*s).to_owned()), k7x::PathSeg::Index)
        })
        .collect();
    locate(&root, &path).map(|at| line_col(text, at))
}

#[test]
fn a_findings_position_is_the_keys_line_and_column() {
    // k7xPosition.test.ts: is the key's own line and column in an object, and in a chain alike.
    let object = "project({\n  name: \"demo\",\n  tempo: \"fast\",\n})\n";
    let chain = "steps([\n  id(1),\n  id(2).to([\"x\"]),\n])\n";
    assert_eq!(pos(object, &["project", "tempo"]), Some((3, 3)));
    assert_eq!(pos(object, &["project", "name"]), Some((2, 3)));
    assert_eq!(pos(chain, &["steps", "1", "to"]), Some((3, 9)));
    assert_eq!(pos(chain, &["steps", "1"]), Some((3, 3)));
}

#[test]
fn a_position_falls_back_to_the_deepest_part_that_exists() {
    // k7xPosition.test.ts: falls back to the deepest part of the path that exists.
    let object = "project({\n  name: \"demo\",\n  tempo: \"fast\",\n})\n";
    let chain = "steps([\n  id(1),\n  id(2).to([\"x\"]),\n])\n";
    assert_eq!(pos(chain, &["steps", "1", "nope", "4"]), Some((3, 3)));
    assert_eq!(pos(object, &["nothing"]), None);
    assert_eq!(pos(object, &[]), None);
}

#[test]
fn a_column_counts_utf16_units() {
    let text = "name(\"é😀\")\nx(1)\n";
    assert_eq!(line_col(text, text.find("x(").expect("there")), (2, 1));
    assert_eq!(line_col("é😀x", "é😀".len()), (1, 4));
}

#[test]
fn a_clip_kind_is_told_by_the_extension_of_its_source() {
    let audio = one::<Clip>("clips/a.k7x", "source(\"kick.wav\")\nfrom(0.25)\n").expect("reads");
    assert!(matches!(&*audio, Clip::Audio(_)));
    let video = one::<Clip>("clips/v.k7x", "source(\"x.MP4\")\nsound(\"x.wav\")\n").expect("reads");
    assert!(matches!(&*video, Clip::Video(_)));
    let still = one::<Clip>("clips/s.k7x", "source(\"x.jpeg\")\n").expect("reads");
    assert!(matches!(&*still, Clip::Video(_)));
    let caption = one::<Clip>("clips/c.k7x", "source(\"x.srt\")\nalign(\"left\")\nfont_size(24)\n").expect("reads");
    assert!(matches!(&*caption, Clip::Caption(_)));
    assert!(one::<Clip>("clips/v.k7x", "source(\"x.mp4\")\nsound(\"y.mkv\")\n").is_err());
    assert!(one::<Clip>("clips/a.k7x", "source(\"kick.wav\")\nsound(\"y.wav\")\n").is_err());
    assert!(one::<Clip>("clips/c.k7x", "source(\"x.srt\")\nalign(\"up\")\n").is_err());
}

#[test]
fn a_picture_source_with_audio_fields_is_the_audio_clip_the_union_accepts() {
    // The TypeScript union tries video, captions, then audio: a video file with a window is an audio clip.
    let c = one::<Clip>("clips/v.k7x", "source(\"x.mp4\")\nfrom(1)\n").expect("reads");
    assert!(matches!(&*c, Clip::Audio(_)));
}

#[test]
fn a_window_that_cannot_be_played_is_refused_naming_the_field() {
    // clipWindow.test.ts: is refused by the schema, naming the field; and reported against its own file.
    let f = one::<Clip>("clips/kick.k7x", "source(\"clips/kick.wav\")\nfrom(2)\nto(1)\n").expect_err("refused");
    assert_eq!((f.line, f.col), (3, 1));
}

#[test]
fn refuses_a_negative_from_and_accepts_one_edge_or_none() {
    // clipWindow.test.ts: refuses a negative `from`; accepts a window with only one edge, or none.
    assert!(one::<Clip>("clips/k.k7x", "source(\"a.wav\")\nfrom(-1)\n").is_err());
    for text in [
        "source(\"a.wav\")\n",
        "source(\"a.wav\")\nfrom(0.5)\n",
        "source(\"a.wav\")\nto(0.5)\n",
    ] {
        assert!(one::<Clip>("clips/k.k7x", text).is_ok(), "{text}");
    }
    assert!(one::<Clip>("clips/k.k7x", "source(\"a.wav\")\nfrom(\"nonsense\")\n").is_err());
    assert!(one::<Clip>("clips/k.k7x", "source(\"a.wav\")\nfrom(\"2:1\")\nto(\"3:1\")\n").is_ok());
}

#[test]
fn an_unknown_field_is_placed_on_its_key_and_a_wrong_type_on_its_value() {
    let f = one::<Clip>("clips/k.k7x", "source(\"a.wav\")\nnonsense(1)\n").expect_err("refused");
    assert_eq!((f.line, f.col), (2, 1));
    let f = one::<Clip>("clips/k.k7x", "source(\"a.wav\")\nfrom(true)\n").expect_err("refused");
    assert_eq!(f.line, 2);
}

#[test]
fn a_syntax_error_is_a_finding_at_its_position() {
    let f = one::<Clip>("clips/k.k7x", "source(\"a.wav\"\n").expect_err("refused");
    assert!(f.line >= 1 && !f.message.is_empty());
}

#[test]
fn a_timeline_reads_and_refuses_what_its_schema_refuses() {
    let ok =
        "length(\"8:0\")\nlanes([\n  lane(\"drums\").events([ref(\"kick\").start(\"1:1\").fadeIn(0.5).loop(2)]),\n])\n";
    let t = one::<Timeline>("timelines/t.k7x", ok).expect("reads");
    assert_eq!(t.lanes.len(), 1);
    let twice = "lanes([lane(\"a\"), lane(\"a\")])\n";
    assert!(one::<Timeline>("timelines/t.k7x", twice).is_err());
    let picture_twice = "lanes([lane(\"a\")])\npictures([lane(\"a\")])\n";
    assert!(one::<Timeline>("timelines/t.k7x", picture_twice).is_err());
    assert!(one::<Timeline>("timelines/t.k7x", "lanes([lane(\"a b\")])\n").is_err());
    let backwards = "lanes([lane(\"a\").events([ref(\"k\").from(2).to(1)])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", backwards).is_err());
    let activation = "lanes([lane(\"a\").events([start(\"1:1\").fadeIn(1)])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", activation).is_err());
    let bare = "lanes([lane(\"a\").events([start(\"1:1\")])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", bare).is_ok());
}

#[test]
fn a_shot_running_out_of_source_backwards_is_refused() {
    let shot = "lanes([lane(\"a\").events([ref(\"k\").speed(-1).length(5).from(1)])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", shot).is_err());
    let fine = "lanes([lane(\"a\").events([ref(\"k\").speed(-1).length(1).from(2)])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", fine).is_ok());
}

#[test]
fn a_fade_is_a_timecode_seconds_widened_to_a_map_or_a_map() {
    let text = "lanes([lane(\"a\").events([ref(\"k\").fadeIn(\"2:0\").fadeOut(0.5)])])\n";
    let t = one::<Timeline>("timelines/t.k7x", text).expect("reads");
    let json = serde_json::to_value(&*t).expect("json");
    assert_eq!(json["lanes"][0]["events"][0]["fadeIn"], "2:0");
    assert_eq!(json["lanes"][0]["events"][0]["fadeOut"]["over"], 0.5);
    let map = "lanes([lane(\"a\").events([ref(\"k\").fadeIn(over(1).with(\"wipe\"))])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", map).is_ok());
}

#[test]
fn an_animation_is_a_constant_or_keys() {
    let ok = "lanes([lane(\"a\").events([ref(\"k\").animate(x(4).opacity([[0,0],[1,1]]).scale([t(0).v(1).ease(\"inout\")]))])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", ok).is_ok());
    let empty = "lanes([lane(\"a\").events([ref(\"k\").animate(x([]))])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", empty).is_err());
    let typo = "lanes([lane(\"a\").events([ref(\"k\").animate(skew(1))])])\n";
    assert!(one::<Timeline>("timelines/t.k7x", typo).is_err());
}
