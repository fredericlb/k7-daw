// pictures.rs — M6's shot verbs on the video-lane fixture: every write read back, undone, and every refusal writing nothing.
// A reader would get wrong the lane's text: `nodes/main.k7x` is rewritten by the lane's own writer, so its comments must survive.

mod support;

use song_edit::Fs;
use support::Project;

const TWO: &str = "extends(\"@meta/picture-track\")\n// the camera\nevents([\n  ref(\"sample\").start(0).length(4),\n  ref(\"sample\").start(4).length(6).from(2.5),\n])\nframe([0, 0, 1280, 720])\n";

fn video() -> Project {
    let mut p = Project::open("video-lane");
    let path = format!("{}/nodes/main.k7x", p.dir);
    p.fs.write_atomic(&path, TWO.as_bytes()).expect("a write");
    p
}

fn shot(p: &Project, i: usize, field: &str) -> f64 {
    match p.data("nodes/main.k7x").get("events") {
        Some(k7x::edit::Value::List(l)) => match l[i].get(field) {
            Some(k7x::edit::Value::Num(n)) => *n,
            _ => f64::NAN,
        },
        _ => f64::NAN,
    }
}

#[test]
fn a_shot_moves_and_the_comments_stay() {
    let mut p = video();
    let ran = p.round_trip("picture move main 4 10");
    assert_eq!(ran.changed(&p.dir), ["nodes/main.k7x"]);
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample 4s → 10s"]);
    p.ok("picture move main 4 10");
    assert_eq!(shot(&p, 1, "start"), 10.0);
    assert!(p.text("nodes/main.k7x").contains("// the camera"));
    p.ok("picture move main 0 1:30.5");
    assert_eq!(shot(&p, 0, "start"), 90.5);
}

#[test]
fn a_time_that_hits_nothing_names_the_starts_there_are() {
    let mut p = video();
    p.refused(
        "picture move main 7 9",
        "nothing starts at 7s on nodes/main.k7x — there are shots at 0, 4",
    );
    p.refused("picture move main 2:1:1 9", "not a time — picture reads seconds");
    p.refused("picture move nope 0 4", "no picture lane 'nope' — this song's are main");
    p.refused("picture move main", "usage: picture move <lane> <at> <to>");
    p.refused("picture move drums 0 4", "no picture lane 'drums'");
}

#[test]
fn two_shots_at_one_instant_cannot_be_told_apart() {
    let mut p = video();
    let same =
        "extends(\"@meta/picture-track\")\nevents([ref(\"a\").start(4).length(1), ref(\"b\").start(4).length(1)])\n";
    let path = format!("{}/nodes/main.k7x", p.dir);
    p.fs.write_atomic(&path, same.as_bytes()).expect("a write");
    p.refused("picture move main 4 8", "2 shots start at 4s on nodes/main.k7x");
}

#[test]
fn a_head_trim_moves_three_fields_and_holds_the_tail() {
    let mut p = video();
    let ran = p.round_trip("picture head main 4 6");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample head 4s → 6s, tail held"]);
    p.ok("picture head main 4 6");
    assert_eq!(
        (shot(&p, 1, "start"), shot(&p, 1, "length"), shot(&p, 1, "from")),
        (6.0, 4.0, 4.5)
    );
    p.refused("picture head main 0 -1", "is not a time");
    p.refused(
        "picture head main 6 10",
        "that would leave sample 0ms long — trim the tail instead",
    );
    let before = p.files();
    let ran = p.ok("picture head main 6 6");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample unchanged"]);
    assert_eq!(p.files(), before);
}

#[test]
fn a_head_cannot_walk_back_past_the_first_frame() {
    let mut p = video();
    p.refused(
        "picture head main 4 0",
        "sample starts 2.5s into its source, so the head cannot move back more than that",
    );
}

#[test]
fn length_delete_and_add_edit_one_placement_each() {
    let mut p = video();
    p.round_trip("picture length main 0 2.5");
    p.ok("picture length main 0 2.5");
    assert_eq!(shot(&p, 0, "length"), 2.5);
    let ran = p.ok("picture length main 4 0");
    assert_eq!(
        ran.answer.log,
        ["nodes/main.k7x: sample lasts to the end of its source"]
    );
    p.round_trip("picture delete main 0");
    p.ok("picture delete main 0");
    assert_eq!(shot(&p, 0, "start"), 4.0);
    let ran = p.round_trip("picture add main shot_c 12.5 4 1");
    assert_eq!(
        ran.answer.log,
        [
            "nodes/main.k7x: + shot_c at 12.5s for 4s",
            "no clips/shot_c.k7x — the lane will report it until there is one"
        ]
    );
    p.ok("picture add main shot_c 12.5 4 1");
    assert_eq!(shot(&p, 1, "from"), 1.0);
    p.refused("picture add main", "usage: picture add <lane> <ref> <at>");
}

#[test]
fn a_split_makes_two_placements_of_one_clip() {
    let mut p = video();
    let ran = p.round_trip("picture split main 4 7");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample at 4s split at 7s"]);
    p.ok("picture split main 4 7");
    assert_eq!(shot(&p, 1, "length"), 3.0);
    assert_eq!(
        (shot(&p, 2, "start"), shot(&p, 2, "length"), shot(&p, 2, "from")),
        (7.0, 3.0, 5.5)
    );
    p.refused(
        "picture split main 0 4",
        "nodes/main.k7x: 4s is not inside the shot at 0s",
    );
}

#[test]
fn a_rect_states_all_four_numbers() {
    let mut p = video();
    let ran = p.round_trip("picture rect main 960 540 320 180");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: 320x180 at 960,540"]);
    p.ok("picture rect main 960 540 320 180");
    assert!(
        p.text("nodes/main.k7x").contains("frame([960, 540, 320, 180])"),
        "{}",
        p.text("nodes/main.k7x")
    );
    p.refused("picture rect main 0 0 0 10", "a lane 0x10 draws nothing");
    p.refused("picture rect main 0 0 10", "usage: picture rect");
    p.refused(
        "picture rect verse>main 0 0 10 10",
        "verse>main draws on the song's picture lane 'main' — rect that one",
    );
}

#[test]
fn a_shot_is_keyed_sped_and_silenced() {
    let mut p = video();
    p.round_trip("picture key main 4 opacity 5 0.5");
    let ran = p.ok("picture key main 4 opacity 5 0.5 inout");
    assert_eq!(
        ran.answer.log,
        ["nodes/main.k7x: sample opacity = 0.5 at 5s (1s into the shot)"]
    );
    p.ok("picture key main 4 x 4 0");
    let ran = p.round_trip("picture unkey main 4 x 4");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample − x key at 4s"]);
    p.refused("picture unkey main 4 x 9", "no x key at 9s on the shot at 4s");
    p.refused(
        "picture key main 4 zoom 5 1",
        "is not something a shot animates — x, y, scale, opacity, rotate",
    );
    p.refused("picture key main 4 x 5 1 wobble", "is not an ease");
    p.round_trip("picture speed main 4 2");
    p.ok("picture speed main 4 0.5");
    assert_eq!(shot(&p, 1, "speed"), 0.5);
    p.ok("picture speed main 4 1");
    assert!(shot(&p, 1, "speed").is_nan());
    p.refused("picture speed main 4 -2", "runs out of source after 1.25s");
    p.refused("picture speed main 4 fast", "is not a speed");
    let ran = p.round_trip("picture silence main 4");
    assert_eq!(
        ran.answer.log,
        ["nodes/main.k7x: sample at 4s silenced — its picture stays"]
    );
    p.ok("picture silence main 4");
    let ran = p.ok("picture silence main 4");
    assert_eq!(ran.answer.log, ["nodes/main.k7x: sample at 4s heard again"]);
}

#[test]
fn a_lane_hides_and_solos_through_the_graph_and_the_root() {
    let mut p = video();
    let ran = p.round_trip("picture mute main");
    assert_eq!(ran.answer.log, ["main: hidden (graph.k7x)"]);
    p.ok("picture mute main");
    let ran = p.ok("picture mute main");
    assert_eq!(ran.answer.log, ["main: shown (graph.k7x)"]);
    let ran = p.round_trip("picture solo main");
    assert_eq!(ran.answer.log, ["main: solo on — soloed: main"]);
    p.ok("picture solo main");
    let ran = p.ok("picture solo main");
    assert_eq!(ran.answer.log, ["main: solo off"]);
    p.refused("picture mute nope", "no picture lane 'nope'");
}

fn animated() -> Project {
    let mut p = Project::open("video-lane");
    let lane = "extends(\"@meta/picture-track\")\nevents([\n  { ref: \"sample\", start: 0, length: 4, animate: { x: [[0, 0], [4, 40]], scale: 2 } },\n])\nframe([0, 0, 1280, 720])\n";
    let path = format!("{}/nodes/main.k7x", p.dir);
    p.fs.write_atomic(&path, lane.as_bytes()).expect("a write");
    p
}

#[test]
fn a_split_cuts_the_animation_exactly_between_the_halves() {
    let mut p = animated();
    p.round_trip("picture split main 0 1");
    p.ok("picture split main 0 1");
    let text = p.text("nodes/main.k7x");
    assert!(
        text.contains("x: [[0, 0], [4, 40]]"),
        "the head keeps the keys that shape it: {text}"
    );
    assert!(
        text.contains("x: [[-1, 0], [3, 40]]"),
        "the tail is the same keys shifted: {text}"
    );
    assert_eq!((text.matches("scale: 2").count(), shot(&p, 1, "from")), (2, 1.0));
}

#[test]
fn a_head_trim_keeps_the_animation_on_the_frames_it_was_set_on() {
    let mut p = animated();
    p.round_trip("picture head main 0 1");
    p.ok("picture head main 0 1");
    assert!(
        p.text("nodes/main.k7x").contains("x: [[-1, 0], [3, 40]]"),
        "{}",
        p.text("nodes/main.k7x")
    );
    assert_eq!(
        (shot(&p, 0, "start"), shot(&p, 0, "length"), shot(&p, 0, "from")),
        (1.0, 3.0, 1.0)
    );
}

#[test]
fn a_lone_key_at_zero_is_written_back_as_the_constant_it_means() {
    let mut p = Project::open("video-lane");
    let opacity = |p: &Project| match p.data("nodes/main.k7x").get("events") {
        Some(k7x::edit::Value::List(l)) => l[0].get("animate").and_then(|a| a.get("opacity")).cloned(),
        _ => None,
    };
    p.ok("picture key main 0 opacity 0 0.5");
    assert_eq!(opacity(&p), Some(k7x::edit::Value::Num(0.5)));
    p.ok("picture key main 0 opacity 2 1 inout");
    let Some(k7x::edit::Value::List(keys)) = opacity(&p) else {
        panic!("two keys are a list");
    };
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[1].get("ease"), Some(&k7x::edit::Value::Str("inout".to_owned())));
    p.ok("picture unkey main 0 opacity 2");
    assert_eq!(opacity(&p), Some(k7x::edit::Value::Num(0.5)));
    p.ok("picture unkey main 0 opacity 0");
    assert_eq!(opacity(&p), None);
}
