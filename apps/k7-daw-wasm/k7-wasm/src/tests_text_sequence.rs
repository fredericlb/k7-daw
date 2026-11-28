// tests_text_sequence.rs — addressing ONE note in a sequence and rewriting it, what the piano roll commits.
// The file is hand-edited, so the shape is pinned as hard as the value: one note per line, the others untouched, no `v(1)` written.

use crate::tests_support::{call, data, edited, refused};
use serde_json::{Value, json};

const AUTHORED: &str =
    "from(\"1:1\")\nnotes([\n  t(0).d(1).k(60),\n  t(1).d(0.5).k(64).v(0.4),\n  t(1).d(0.5).k(67),\n])\n";

fn run(text: &str, op: Value) -> Result<String, String> {
    edited("text.sequence", json!({"text": text, "ops": [op]})).map(|(t, _)| t)
}

fn edit(text: &str, op: Value) -> String {
    run(text, op).unwrap()
}

fn notes_of(text: &str) -> Value {
    data(text)["notes"].clone()
}

fn error(text: &str, op: Value) -> String {
    run(text, op).unwrap_err()
}

fn valid(text: &str) -> bool {
    let v = call("file.validate", json!({"path": "sequences/s.k7x", "text": text}));
    v["ok"] == json!(true)
}

#[test]
fn a_short_form_is_read_as_the_notes_it_expands_to() {
    let d = call("text.sequence_data", json!({"text": "notes(\"c4 e4\")\n"}));
    assert_eq!(
        d["notes"],
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 1, "k": 64}])
    );
    assert!(!refused("text.sequence_data", json!({"text": "notes(["})).is_empty());
}

#[test]
fn remove_note_removes_exactly_the_note_addressed_and_leaves_the_others_alone() {
    let after = edit(AUTHORED, json!({"op": "removeNote", "k": 64, "t": 1}));
    assert_eq!(
        notes_of(&after),
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 0.5, "k": 67}])
    );
    assert!(valid(&after));
    assert!(edit(AUTHORED, json!({"op": "removeNote", "k": 60, "t": 0})).contains("from(\"1:1\")"));
}

#[test]
fn two_stacked_notes_are_told_apart_by_their_key() {
    let after = edit(AUTHORED, json!({"op": "removeNote", "k": 67, "t": 1}));
    assert_eq!(
        notes_of(&after),
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 0.5, "k": 64, "v": 0.4}])
    );
}

#[test]
fn a_note_it_cannot_find_is_refused_rather_than_removing_something_else() {
    assert!(error(AUTHORED, json!({"op": "removeNote", "k": 60, "t": 9})).contains("no note"));
}

fn velocity(k: i32, t: f64, v: Option<f64>) -> Value {
    let mut op = json!({"op": "setNoteVelocity", "k": k, "t": t});
    if let Some(v) = v {
        op["v"] = json!(v);
    }
    op
}

#[test]
fn a_velocity_is_written_leaving_the_other_notes_alone_in_a_one_line_diff() {
    let after = edit(AUTHORED, velocity(60, 0.0, Some(0.75)));
    assert_eq!(
        notes_of(&after),
        json!([{"t": 0, "d": 1, "k": 60, "v": 0.75}, {"t": 1, "d": 0.5, "k": 64, "v": 0.4}, {"t": 1, "d": 0.5, "k": 67}])
    );
    assert_eq!(after, AUTHORED.replace("k(60)", "k(60).v(0.75)"));
    assert!(valid(&after));
}

#[test]
fn a_velocity_the_note_already_had_is_replaced_and_none_clears_the_key_not_the_default() {
    assert_eq!(
        notes_of(&edit(AUTHORED, velocity(64, 1.0, Some(0.9))))[1],
        json!({"t": 1, "d": 0.5, "k": 64, "v": 0.9})
    );
    let after = edit(AUTHORED, velocity(64, 1.0, None));
    assert_eq!(notes_of(&after)[1], json!({"t": 1, "d": 0.5, "k": 64}));
    assert!(!after.contains("v("));
}

#[test]
fn a_velocity_for_a_missing_note_is_refused_and_the_comments_are_kept() {
    assert!(error(AUTHORED, velocity(60, 9.0, Some(0.5))).contains("no note"));
    let with_comments = "// the melody\nnotes([\n  // the downbeat\n  t(0).d(1).k(60),\n])\n";
    let after = edit(with_comments, velocity(60, 0.0, Some(0.5)));
    assert!(after.contains("// the melody"));
    assert!(after.contains("// the downbeat"));
}

#[test]
fn a_short_form_is_kept_while_an_edit_leaves_its_notes_as_they_were_and_written_in_full_once_it_changes_them() {
    let text = "notes(\"c4 e4\")\n";
    assert_eq!(edit(text, velocity(60, 0.0, None)), text);
    let after = edit(text, velocity(64, 1.0, Some(0.5)));
    assert_eq!(
        notes_of(&after),
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 1, "k": 64, "v": 0.5}])
    );
}

#[test]
fn an_unreadable_text_is_an_error() {
    assert!(run("notes([", json!({"op": "removeNote", "k": 1, "t": 0})).is_err());
}

#[test]
fn set_notes_writes_the_whole_list_changing_only_the_notes_that_changed() {
    let next = json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 0.5, "k": 64, "v": 0.9}, {"t": 1, "d": 0.5, "k": 67}]);
    assert_eq!(
        edit(AUTHORED, json!({"op": "setNotes", "notes": next})),
        AUTHORED.replace("v(0.4)", "v(0.9)")
    );
}

#[test]
fn set_notes_keeps_a_short_form_that_the_new_notes_still_say() {
    let text = "notes(\"c4 e4\")\nto(\"2:1\")\n";
    let same = json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 1, "k": 64}]);
    assert_eq!(edit(text, json!({"op": "setNotes", "notes": same})), text);
}

#[test]
fn the_splices_a_note_edit_made_replay_onto_the_text_before() {
    let notes = json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 0.5, "k": 64, "v": 0.9}, {"t": 1, "d": 0.5, "k": 67}]);
    let v = call(
        "text.sequence",
        json!({"text": AUTHORED, "ops": [{"op": "setNotes", "notes": notes}]}),
    );
    let changes = v["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    let units: Vec<u16> = AUTHORED.encode_utf16().collect();
    let (s, e) = (
        changes[0]["start"].as_u64().unwrap() as usize,
        changes[0]["end"].as_u64().unwrap() as usize,
    );
    let mut out = String::from_utf16(&units[..s]).unwrap();
    out.push_str(changes[0]["insert"].as_str().unwrap());
    out.push_str(&String::from_utf16(&units[e..]).unwrap());
    assert_eq!(out, v["text"].as_str().unwrap());
}

fn mv(k: i32, t: f64, new_k: i32, new_t: f64) -> Result<String, String> {
    run(
        AUTHORED,
        json!({"op": "moveNote", "k": k, "t": t, "newK": new_k, "newT": new_t}),
    )
}

#[test]
fn a_moved_note_carries_its_length_and_velocity_to_the_new_key_and_start() {
    let after = mv(64, 1.0, 60, 2.5).unwrap();
    assert_eq!(
        notes_of(&after),
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 2.5, "d": 0.5, "k": 60, "v": 0.4}, {"t": 1, "d": 0.5, "k": 67}])
    );
    assert!(valid(&after));
}

#[test]
fn a_note_moves_within_one_key_and_to_another_key_at_the_same_beat() {
    assert_eq!(
        notes_of(&mv(60, 0.0, 60, 3.0).unwrap())[0],
        json!({"t": 3, "d": 1, "k": 60})
    );
    assert_eq!(
        notes_of(&mv(60, 0.0, 62, 0.0).unwrap())[0],
        json!({"t": 0, "d": 1, "k": 62})
    );
}

#[test]
fn a_move_onto_a_held_address_is_refused_one_onto_itself_accepted_a_missing_note_or_early_start_refused() {
    assert!(mv(60, 0.0, 67, 1.0).unwrap_err().contains("already"));
    assert_eq!(
        notes_of(&mv(60, 0.0, 60, 0.0).unwrap())[0],
        json!({"t": 0, "d": 1, "k": 60})
    );
    assert!(mv(60, 9.0, 61, 0.0).unwrap_err().contains("no note"));
    assert!(mv(60, 0.0, 61, -1.0).unwrap_err().contains("before"));
}

fn moving(moves: Value) -> Result<String, String> {
    run(AUTHORED, json!({"op": "moveNotes", "moves": moves}))
}

#[test]
fn move_notes_moves_every_note_in_one_pass_each_to_its_own_place() {
    let out = moving(json!([{"k": 60, "t": 0, "nk": 62, "nt": 2}, {"k": 64, "t": 1, "nk": 65, "nt": 3}])).unwrap();
    assert_eq!(
        notes_of(&out),
        json!([{"t": 2, "d": 1, "k": 62}, {"t": 3, "d": 0.5, "k": 65, "v": 0.4}, {"t": 1, "d": 0.5, "k": 67}])
    );
}

#[test]
fn a_selection_shifted_onto_itself_lands_on_a_place_another_moving_note_leaves() {
    let out = moving(json!([{"k": 64, "t": 1, "nk": 67, "nt": 1}, {"k": 67, "t": 1, "nk": 67, "nt": 2}])).unwrap();
    assert_eq!(
        notes_of(&out),
        json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 0.5, "k": 67, "v": 0.4}, {"t": 2, "d": 0.5, "k": 67}])
    );
}

#[test]
fn move_notes_refuses_what_it_cannot_do_writing_nothing() {
    assert!(
        moving(json!([{"k": 64, "t": 1, "nk": 60, "nt": 0}]))
            .unwrap_err()
            .contains("already")
    );
    let two = json!([{"k": 60, "t": 0, "nk": 70, "nt": 4}, {"k": 64, "t": 1, "nk": 70, "nt": 4}]);
    assert!(moving(two).unwrap_err().contains("two notes"));
    assert!(
        moving(json!([{"k": 61, "t": 0, "nk": 62, "nt": 0}]))
            .unwrap_err()
            .contains("no note")
    );
    assert!(
        moving(json!([{"k": 60, "t": 0, "nk": 60, "nt": -1}]))
            .unwrap_err()
            .contains("before")
    );
    assert!(moving(json!([])).unwrap_err().contains("no note"));
}

const TWO: &str = "notes([\n  t(0).d(1).k(60),\n  t(4).d(2).k(64).v(0.7),\n])\n";

fn range(k: i32, t: f64, new_t: f64, new_d: f64) -> Result<String, String> {
    run(
        TWO,
        json!({"op": "setNoteTimeRange", "k": k, "t": t, "newT": new_t, "newD": new_d}),
    )
}

#[test]
fn a_time_range_moves_a_note_keeping_what_else_it_carries_and_stretches_keeping_the_start() {
    let out = range(64, 4.0, 6.0, 2.0).unwrap();
    assert_eq!(notes_of(&out)[1], json!({"t": 6, "d": 2, "k": 64, "v": 0.7}));
    assert!(out.contains("  t(0).d(1).k(60),\n"));
    assert_eq!(
        notes_of(&range(60, 0.0, 0.0, 3.0).unwrap())[0],
        json!({"t": 0, "d": 3, "k": 60})
    );
}

#[test]
fn a_time_range_refuses_what_it_cannot_write() {
    let e = range(60, 99.0, 0.0, 1.0).unwrap_err();
    assert!(e.contains("k60") && e.contains("99"), "{e}");
    assert!(range(60, 0.0, 0.0, 0.0).unwrap_err().contains("longer than"));
    assert!(range(60, 0.0, 0.0, -1.0).unwrap_err().contains("longer than"));
    assert!(range(60, 0.0, -1.0, 1.0).unwrap_err().contains("before"));
}

fn split(k: i32, t: f64, at: f64) -> Result<String, String> {
    run(TWO, json!({"op": "splitNote", "k": k, "t": t, "at": at}))
}

#[test]
fn a_split_shortens_the_first_half_and_writes_the_second_the_velocity_on_both() {
    let out = split(64, 4.0, 5.0).unwrap();
    assert_eq!(
        notes_of(&out).as_array().unwrap()[1..],
        [
            json!({"t": 4, "d": 1, "k": 64, "v": 0.7}),
            json!({"t": 5, "d": 1, "k": 64, "v": 0.7})
        ]
    );
    assert!(out.contains("  t(0).d(1).k(60),\n"));
    let plain = notes_of(&split(60, 0.0, 0.25).unwrap());
    assert_eq!(plain[0], json!({"t": 0, "d": 0.25, "k": 60}));
    assert_eq!(plain[2], json!({"t": 0.25, "d": 0.75, "k": 60}));
}

#[test]
fn a_split_refuses_a_missing_note_and_a_cut_on_or_outside_an_edge_naming_where_the_note_runs() {
    assert!(split(60, 99.0, 99.5).unwrap_err().contains("k60"));
    for at in [4.0, 6.0, 9.0, 1.0] {
        assert!(split(64, 4.0, at).unwrap_err().contains("inside the note"), "{at}");
    }
    assert!(split(64, 4.0, 6.0).unwrap_err().contains("4 to 6"));
}

const PLAIN: &str = "notes([\n  t(0).d(1).k(60),\n  t(2.5).d(0.5).k(64).v(0.8),\n])\n";
const WINDOWED: &str = "from(\"1:1\")\nto(\"3:1\")\nnotes([\n  t(0).d(1).k(60).repeat(2),\n  t(9).d(1).k(62),\n])\n";

fn double(text: &str, num: u32, den: u32) -> Result<String, String> {
    run(text, json!({"op": "doubleSequence", "num": num, "den": den}))
}

#[test]
fn doubling_copies_every_note_one_bar_rounded_extent_later_writing_no_window() {
    let out = double(PLAIN, 4, 4).unwrap();
    assert_eq!(
        notes_of(&out).as_array().unwrap()[2..],
        [
            json!({"t": 4, "d": 1, "k": 60}),
            json!({"t": 6.5, "d": 0.5, "k": 64, "v": 0.8})
        ]
    );
    assert!(data(&out).get("to").is_none());
    assert!(out.contains("  t(4).d(1).k(60),\n"));
}

#[test]
fn the_bar_is_read_off_the_signature_so_three_four_doubles_by_three_beats() {
    assert_eq!(
        notes_of(&double(PLAIN, 3, 4).unwrap())[2],
        json!({"t": 3, "d": 1, "k": 60})
    );
}

#[test]
fn a_stated_to_moves_out_by_the_windows_length_copying_what_the_window_holds_with_its_effects() {
    let out = double(WINDOWED, 4, 4).unwrap();
    assert_eq!(data(&out)["to"], json!("5:1"));
    let notes = notes_of(&out);
    let notes = notes.as_array().unwrap();
    assert_eq!(notes[2], json!({"t": 8, "d": 1, "k": 60, "repeat": 2}));
    assert_eq!(notes.iter().filter(|n| n["k"] == json!(62)).count(), 1);
    assert_eq!(notes.iter().filter(|n| n["repeat"] == json!(2)).count(), 2);
}

#[test]
fn doubling_refuses_an_empty_sequence_and_a_window_with_nothing_in_it() {
    assert!(double("notes([])\n", 4, 4).unwrap_err().contains("no notes"));
    let e = double("from(\"1:1\")\nto(\"2:1\")\nnotes([t(8).d(1).k(60)])\n", 4, 4).unwrap_err();
    assert!(e.contains("nothing to double"), "{e}");
}

#[test]
fn appended_notes_get_a_line_each_with_their_effects_and_the_list_is_made_when_absent() {
    let out = edit(
        "from(\"1:1\")\n",
        json!({"op": "appendNotes", "notes": [{"t": 0, "d": 1, "k": 60, "repeat": 2}]}),
    );
    assert_eq!(notes_of(&out), json!([{"t": 0, "d": 1, "k": 60, "repeat": 2}]));
    let more = edit(
        AUTHORED,
        json!({"op": "appendNotes", "notes": [{"t": 4, "d": 1, "k": 72, "v": 0.5}]}),
    );
    assert!(more.contains("  t(4).d(1).k(72).v(0.5),\n"));
}

#[test]
fn a_pad_effect_is_turned_on_off_and_a_count_that_says_nothing_refused() {
    let on = edit(
        AUTHORED,
        json!({"op": "setNoteEffect", "k": 60, "t": 0, "effect": "repeat", "n": 3}),
    );
    assert_eq!(notes_of(&on)[0], json!({"t": 0, "d": 1, "k": 60, "repeat": 3}));
    let off = edit(&on, json!({"op": "setNoteEffect", "k": 60, "t": 0, "effect": "repeat"}));
    assert_eq!(off, AUTHORED);
    let e = error(
        AUTHORED,
        json!({"op": "setNoteEffect", "k": 60, "t": 0, "effect": "skip", "n": 1}),
    );
    assert!(e.contains("whole number of 2 or more"), "{e}");
}
