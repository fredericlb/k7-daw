// tests_text_video.rs — a picture lane's writers pinned by the text they leave: a split is two placements that play as the one did.
// A reader would get wrong that a refusal is an answer: `false` in `results` and the text as it was.

use crate::tests_support::{call, data, edited};
use serde_json::{Value, json};

/// One op over a whole file's text (or the lane at `at`): the text and what the writer answered.
fn edit_at(text: &str, op: Value, at: Value) -> (String, bool) {
    let (t, r) = edited("text.video", json!({"text": text, "at": at, "ops": [op]})).unwrap();
    (t, r[0])
}

fn edit(text: &str, op: Value) -> (String, bool) {
    edit_at(text, op, json!([]))
}

fn events(text: &str) -> Value {
    data(text)["events"].clone()
}

fn listed(text: &str, at: Value) -> Value {
    call("text.video_list", json!({"text": text, "at": at}))
}

fn refs(text: &str, at: Value) -> Vec<Value> {
    listed(text, at)
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["ref"].clone())
        .collect()
}

const LANE: &str = "// the A-roll\nextends(\"@meta/picture-track\")\nevents([\n  ref(\"a\").start(2).length(6).from(1).fadeIn(0.5).fadeOut({ over: 1, with: \"wipe\" }),\n  ref(\"b\").start(10).length(0),\n])\n";

fn split(text: &str, index: u32, cut: f64) -> (String, bool) {
    edit(text, json!({"op": "splitAt", "index": index, "cut": cut}))
}

#[test]
fn a_shot_is_cut_into_a_head_and_a_tail_that_play_the_same_frames() {
    let (text, done) = split(LANE, 0, 5.0);
    assert!(done);
    assert_eq!(
        events(&text),
        json!([
            {"ref": "a", "start": 2, "length": 3, "from": 1, "fadeIn": 0.5},
            {"ref": "a", "start": 5, "length": 3, "from": 4, "fadeOut": {"over": 1, "with": "wipe"}},
            {"ref": "b", "start": 10, "length": 0},
        ])
    );
    assert!(text.contains("// the A-roll"));
}

#[test]
fn a_length_0_tail_stays_open_running_to_the_next_start_as_the_shot_did() {
    let (text, _) = split(LANE, 1, 12.0);
    let all = listed(&text, json!([]));
    assert_eq!(
        all.as_array().unwrap()[1..],
        [
            json!({"ref": "b", "start": 10, "length": 2, "sourceFrom": 0}),
            json!({"ref": "b", "start": 12, "length": 0, "sourceFrom": 2}),
        ]
    );
}

#[test]
fn an_activation_is_split_without_inventing_a_source_to_trim() {
    let (text, _) = split("events([start(0).length(4)])\n", 0, 1.0);
    assert_eq!(
        events(&text),
        json!([{"start": 0, "length": 1}, {"start": 1, "length": 3}])
    );
}

#[test]
fn the_animation_is_cut_exactly_each_half_playing_what_the_shot_did() {
    let text = "events([\n  ref(\"a\").start(0).length(8).animate({ x: [[0, 0], [8, 800]], opacity: 0.5 }),\n])\n";
    let out = events(&split(text, 0, 2.0).0);
    assert_eq!(out[0]["animate"], json!({"x": [[0, 0], [8, 800]], "opacity": 0.5}));
    // The tail starts 2 s in, so its keys are the same keys two seconds earlier.
    assert_eq!(out[1]["animate"], json!({"x": [[-2, 0], [6, 800]], "opacity": 0.5}));
}

#[test]
fn a_cut_on_an_edge_or_outside_the_shot_is_refused_leaving_the_file_alone() {
    for (i, cut) in [(0, 2.0), (0, 8.0), (0, 9.0), (5, 3.0)] {
        assert_eq!(split(LANE, i, cut), (LANE.to_owned(), false), "{i} {cut}");
    }
}

#[test]
fn the_animation_keys_move_back_by_a_trimmed_head_and_a_still_shot_is_untouched() {
    let text = "events([\n  ref(\"a\").start(0).length(8).animate({ scale: [[0, 1], { t: 4, v: 2, ease: \"out\" }], x: 10 }),\n])\n";
    let (out, _) = edit(text, json!({"op": "shiftAnimationAt", "index": 0, "delta": 1.5}));
    assert_eq!(
        events(&out)[0]["animate"],
        json!({"scale": [[-1.5, 1], {"t": 2.5, "v": 2, "ease": "out"}], "x": 10})
    );
    let op = json!({"op": "shiftAnimationAt", "index": 0, "delta": 1});
    assert_eq!(edit(LANE, op), (LANE.to_owned(), false));
}

fn add() -> Value {
    json!({"op": "addShot", "ref": "c", "start": 12, "length": 2, "sourceFrom": 0})
}

#[test]
fn a_placement_is_appended_on_a_line_of_its_own_keeping_the_others() {
    let (text, done) = edit(LANE, add());
    assert!(done);
    assert!(text.contains("ref(\"b\").start(10).length(0),"));
    assert_eq!(events(&text)[2], json!({"ref": "c", "start": 12, "length": 2}));
}

#[test]
fn events_is_created_on_an_empty_lane_and_one_holding_something_else_is_refused() {
    let mut op = add();
    op["start"] = json!(0);
    op["length"] = json!(1);
    assert_eq!(events(&edit("", op).0), json!([{"ref": "c", "start": 0, "length": 1}]));
    let odd = "events(\"none\")\n";
    assert_eq!(edit(odd, add()), (odd.to_owned(), false));
}

#[test]
fn a_fades_chain_is_renamed_and_only_a_fades() {
    let text = "events([ref(\"a\").start(0).fadeIn({ over: 1, chain: \"iris\" })])\n";
    let (out, done) = edit(text, json!({"op": "renameFadeChains", "renames": [["iris", "wipe"]]}));
    assert!(done);
    assert!(out.contains("chain: \"wipe\""));
    assert!(!edit(text, json!({"op": "renameFadeChains", "renames": [["x", "y"]]})).1);
}

#[test]
fn the_lanes_rect_is_written_as_frame_all_four_values_in_whole_pixels() {
    let (out, _) = edit(
        LANE,
        json!({"op": "setLaneRect", "x": 10.4, "y": 20, "w": 320, "h": 180.6}),
    );
    assert_eq!(data(&out)["frame"], json!([10, 20, 320, 181]));
    assert!(out.contains("// the A-roll"));
}

#[test]
fn a_half_typed_buffer_lists_nothing_and_edits_nothing() {
    let broken = "events([ref(";
    assert_eq!(listed(broken, json!([])), json!([]));
    assert_eq!(edit(broken, add()), (broken.to_owned(), false));
}

#[test]
fn speed_silence_and_removal_are_set_by_index_the_defaults_leaving_no_key() {
    let text = "events([ref(\"a\").start(0).length(2), ref(\"b\").start(2)])\n";
    let (sped, _) = edit(text, json!({"op": "setSpeedAt", "index": 0, "speed": -0.5}));
    assert_eq!(listed(&sped, json!([]))[0]["speed"], json!(-0.5));
    let (back, _) = edit(&sped, json!({"op": "setSpeedAt", "index": 0, "speed": 1}));
    assert!(!back.contains("speed"));
    let (hushed, _) = edit(text, json!({"op": "setSilentAt", "index": 1, "silent": true}));
    assert_eq!(listed(&hushed, json!([]))[1]["silent"], json!(true));
    assert_eq!(
        edit(&hushed, json!({"op": "setSilentAt", "index": 1, "silent": false})).0,
        text
    );
    assert!(!edit(text, json!({"op": "removeAt", "index": 7})).1);
    let (left, _) = edit(text, json!({"op": "removeAt", "index": 0}));
    assert_eq!(listed(&left, json!([])).as_array().unwrap().len(), 1);
}

#[test]
fn a_timeline_files_lane_is_addressed_at_its_own_key_path() {
    let text = "pictures([\n  { lane: \"a\", events: [ref(\"x\").start(0).length(2)] },\n  { lane: \"b\", events: [ref(\"y\").start(1).length(3)] },\n])\n";
    let at = json!(["pictures", 1]);
    assert_eq!(refs(text, at.clone()), [json!("y")]);
    let (out, done) = edit_at(
        text,
        json!({"op": "setFieldAt", "index": 0, "field": "start", "value": 4.00004}),
        at,
    );
    assert!(done);
    // Rounded to the millisecond, one line changed.
    assert_eq!(out, text.replace("start(1)", "start(4)"));
}

#[test]
fn a_placement_without_a_ref_is_still_listed_so_an_index_addresses_the_same_line() {
    let text = "extends(\"@meta/picture-track\")\nevents([start(0).length(2), ref(\"a\").start(4)])\n";
    assert_eq!(refs(text, json!([])), [json!(""), json!("a")]);
    let (out, done) = edit(
        text,
        json!({"op": "setFieldAt", "index": 1, "field": "start", "value": 6}),
    );
    assert!(done);
    assert!(out.contains("ref(\"a\").start(6)"));
}

#[test]
fn a_field_is_set_by_ref_on_the_first_placement_naming_it() {
    let text = "events([ref(\"a\").start(0).from(1), ref(\"a\").start(5)])\n";
    let (out, _) = edit(
        text,
        json!({"op": "setFieldByRef", "ref": "a", "field": "from", "value": 2.5}),
    );
    assert_eq!(out, text.replace("from(1)", "from(2.5)"));
    assert!(
        !edit(
            text,
            json!({"op": "setFieldByRef", "ref": "z", "field": "from", "value": 1})
        )
        .1
    );
}
