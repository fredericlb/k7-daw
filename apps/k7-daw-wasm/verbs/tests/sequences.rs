// sequences.rs — the sequence verbs on real fixtures: one note, a region, a selection, a take, as the engine then plays them.
// A reader would get wrong a note's address: its KEY and START in beats; `edit-undo`'s melody is C4 E4 G4 C5 on beats 0..3.

mod support;

use song::clips::NoteDoc;
use support::Project;

/// The melody's notes as the engine plays them, as (beat, key, velocity), in time order.
fn notes(p: &mut Project) -> Vec<(f64, u8, f64)> {
    let clip = p.track("melody").clips.into_iter().next().expect("the melody event");
    let mut out: Vec<(f64, u8, f64)> = clip.notes.iter().map(|n: &NoteDoc| (n.time, n.key, n.vel)).collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out
}

fn lengths(p: &mut Project) -> Vec<f64> {
    let clip = p.track("melody").clips.into_iter().next().expect("the melody event");
    let mut n = clip.notes.clone();
    n.sort_by(|a, b| a.time.total_cmp(&b.time));
    n.iter().map(|n| n.duration).collect()
}

#[test]
fn a_note_is_added_and_removed() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence note new melody D4 0.5 1");
    assert_eq!(ran.answer.log, ["added D4 (k62) 0.5..1 → melody"]);
    assert_eq!(ran.changed(&p.dir), ["sequences/melody.k7x"]);
    p.ok("sequence note new melody D4 0.5 1");
    assert!(p.text("sequences/melody.k7x").contains("t(0.5).d(0.5).k(62)"));
    assert!(notes(&mut p).iter().any(|n| n.0 == 0.5 && n.1 == 62));
    p.round_trip("sequence note remove melody D4 0.5");
    p.ok("sequence note remove melody D4 0.5");
    assert_eq!(notes(&mut p).len(), 4);
    p.refused("sequence note remove melody D4 0.5", "no note k62 at beat 0.5");
    p.refused("sequence note new melody H9 0", "<note> expects a note name");
    p.refused("sequence note new nope C4 0", "no sequence file: sequences/nope.k7x");
}

#[test]
fn one_note_moves_stretches_and_splits() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence note timerange melody C4 0 0.5 2");
    assert_eq!(ran.answer.log, ["melody: C4 (k60) 0 → 0.5, length 1.5"]);
    p.ok("sequence note timerange melody C4 0 0.5 2");
    assert_eq!(notes(&mut p)[0].0, 0.5);
    p.refused(
        "sequence note timerange melody C4 0.5 1 1",
        "has to end after it starts",
    );
    p.round_trip("sequence note move melody E4 1 F4 1.5");
    p.ok("sequence note move melody E4 1 F4 1.5");
    assert!(notes(&mut p).iter().any(|n| n.0 == 1.5 && n.1 == 65));
    p.refused(
        "sequence note move melody F4 1.5 G4 2",
        "k67 already has a note at beat 2",
    );
    p.round_trip("sequence note split melody G4 2 2.25");
    p.ok("sequence note split melody G4 2 2.25");
    let g: Vec<_> = notes(&mut p).into_iter().filter(|n| n.1 == 67).collect();
    assert_eq!(g.iter().map(|n| n.0).collect::<Vec<_>>(), [2.0, 2.25]);
    p.refused("sequence note split melody C5 3 4", "is not inside the note");
}

#[test]
fn a_velocity_is_clamped_and_one_clears_it() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence note velocity melody E4 1 0.333");
    assert_eq!(ran.answer.log, ["melody: E4 (k64) at 1 → velocity 0.33"]);
    p.ok("sequence note velocity melody E4 1 0.333");
    assert!((notes(&mut p)[1].2 - 0.33).abs() < 1e-9);
    p.ok("sequence note velocity melody C4 0 7");
    assert!(!p.text("sequences/melody.k7x").contains("v(0.8)"), "1 clears the field");
    p.refused("sequence note velocity melody C4 x 1", "<start> expects a number");
}

#[test]
fn a_pad_repeats_and_skips() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence note repeat melody C4 0 4");
    assert_eq!(ran.answer.log, ["melody: C4 (k60) at 0 → repeated ×4"]);
    p.ok("sequence note repeat melody C4 0 4");
    p.ok("sequence note skip melody E4 1 2");
    let clip = p.track("melody").clips.remove(0);
    assert!(clip.notes.iter().any(|n| n.key == 60 && n.repeat == Some(4)));
    assert!(clip.notes.iter().any(|n| n.key == 64 && n.skip == Some(2)));
    p.ok("sequence note repeat melody C4 0 off");
    assert!(!p.text("sequences/melody.k7x").contains("repeat"));
    p.refused("sequence note skip melody E4 1 1", "a whole number of 2 or more");
}

#[test]
fn a_region_is_edited_in_one_pass() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence notes transpose melody C4 G4 0 3 2");
    assert_eq!(ran.answer.log, ["melody: 3 note(s) transposed"]);
    p.ok("sequence notes transpose melody C4 G4 0 3 2");
    assert_eq!(notes(&mut p).iter().map(|n| n.1).collect::<Vec<_>>(), [62, 66, 69, 72]);
    p.ok("sequence notes velocity melody 0 127 0 4 0.5");
    assert!(notes(&mut p).iter().all(|n| (n.2 - 0.5).abs() < 1e-9));
    p.ok("sequence note timerange melody D4 0 0.3 1.3");
    p.ok("sequence notes quantize melody 0 127 0 1 0.5");
    assert_eq!(notes(&mut p)[0].0, 0.5);
    p.round_trip("sequence notes remove melody 0 127 2 4");
    p.ok("sequence notes remove melody 0 127 2 4");
    assert_eq!(notes(&mut p).len(), 2);
    p.refused(
        "sequence notes remove melody 0 127 8 9",
        "no note between 0 and 127 from beat 8 to 9",
    );
    p.refused(
        "sequence notes quantize melody 0 127 0 4 0",
        "the grid has to be more than 0 beats",
    );
}

#[test]
fn a_selection_moves_as_one() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence notes move melody [[60,0,62,0.5],[64,1,60,0]]");
    assert_eq!(ran.answer.log, ["melody: moved 2 note(s)"]);
    p.ok("sequence notes move melody [[60,0,62,0.5],[64,1,60,0]]");
    assert_eq!(
        &notes(&mut p)[..2].iter().map(|n| (n.0, n.1)).collect::<Vec<_>>(),
        &[(0.0, 60), (0.5, 62)]
    );
    p.refused(
        "sequence notes move melody [[60,0,67,2]]",
        "k67 already has a note at beat 2",
    );
    p.refused(
        "sequence notes move melody [[62,0.5,61,5],[60,0,61,5]]",
        "two notes would land on k61 at beat 5",
    );
    p.refused("sequence notes move melody [1,2]", "usage: sequence notes move");
    p.refused("sequence notes move melody [[", "bad moves JSON");
}

#[test]
fn a_short_form_sequence_is_edited_through_its_expansion() {
    let mut p = Project::open("curves");
    let before = p.track("melody").clips[0].notes.len();
    p.round_trip("sequence note new melody D4 0.5 1");
    p.ok("sequence note new melody D4 0.5 1");
    assert_eq!(p.track("melody").clips[0].notes.len(), before + 1);
}

#[test]
fn doubling_copies_the_window_and_halves_the_loops() {
    let mut p = Project::open("edit-undo");
    p.ok("event loop melody 1:1:1 on");
    let ran = p.round_trip("sequence double melody");
    assert_eq!(
        ran.answer.log[0],
        "melody: doubled — twice as long, its notes played twice"
    );
    p.ok("sequence double melody");
    assert_eq!(notes(&mut p).len(), 8);
    assert_eq!(lengths(&mut p).len(), 8);
    assert!(!p.text("nodes/melody.k7x").contains("loop(2)"), "the loop is halved");
    p.refused("sequence double", "usage: sequence double");
}

#[test]
fn a_new_sequence_is_four_empty_bars_and_the_list_names_the_placed_ones() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence new riff");
    assert_eq!(ran.answer.log, ["+ sequence riff (sequences/riff.k7x)"]);
    p.ok("sequence new riff");
    let data = p.data("sequences/riff.k7x");
    assert_eq!(data.get("to"), Some(&k7x::edit::Value::Str("5:1".to_owned())));
    p.refused("sequence new riff", "sequence 'riff' already exists");
    p.refused("sequence new a/b", "invalid sequence name 'a/b'");
    let list = p.ok("sequence list");
    assert_eq!(list.answer.log, ["  melody  (melody, 4 notes, 4 beats)"]);
    assert_eq!(list.before, list.after);
}

#[test]
fn an_empty_sequence_is_placed_over_a_period() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("sequence empty melody 3:1 5:1");
    assert_eq!(ran.answer.log, ["+ sequence melody-seq-1 on melody at 3:1"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/melody.k7x", "sequences/melody-seq-1.k7x"]);
    p.ok("sequence empty melody 3:1 5:1");
    let placed = p
        .track("melody")
        .clips
        .into_iter()
        .find(|c| c.start == 8.0)
        .expect("placed at bar 3");
    assert_eq!(placed.length, 8.0);
    p.refused("sequence empty melody,ghost 3:1 5:1", "no track \"ghost\"");
    p.refused("sequence empty melody 5:1 3:1", "3:1 is not after 5:1");
}

#[test]
fn a_take_becomes_a_sequence_in_whole_bars() {
    let mut p = Project::open("edit-undo");
    let take = r#"sequence record melody [{"t":5,"d":1,"k":60,"v":0.5},{"t":6.5,"d":0.25,"k":62,"v":1},{"t":"x","d":1,"k":1}]"#;
    let ran = p.round_trip(take);
    assert_eq!(ran.answer.log, ["recorded 2 note(s) → melody-rec-1, on melody at 2:1"]);
    p.ok(take);
    assert!(p.text("sequences/melody-rec-1.k7x").contains("t(1).d(1).k(60).v(0.5)"));
    let placed = p
        .track("melody")
        .clips
        .into_iter()
        .find(|c| c.start == 4.0)
        .expect("placed at bar 2");
    assert_eq!(placed.notes.len(), 2);
    p.refused("sequence record melody [{\"t\":1}]", "no valid notes to record");
    p.refused("sequence record ghost []", "no track \"ghost\"");
}

#[test]
fn recorded_notes_append_with_their_effects_checked() {
    let mut p = Project::open("edit-undo");
    let line = r#"sequence note add melody [{"t":-1,"d":0.5,"k":48,"repeat":3},{"t":4,"d":1,"k":50,"skip":1.5,"v":1}]"#;
    let ran = p.round_trip(line);
    assert_eq!(ran.answer.log, ["recorded 2 note(s) → melody"]);
    p.ok(line);
    let text = p.text("sequences/melody.k7x");
    assert!(text.contains("t(0).d(0.5).k(48).repeat(3)"), "{text}");
    assert!(text.contains("t(4).d(1).k(50)"), "{text}");
    assert!(!text.contains("skip"), "{text}");
    p.refused("sequence note add melody {}", "must be a JSON array");
}
