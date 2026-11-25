// events_add.rs — `events add`, a paste landing: placements on several lanes in one command, each checked as a placement is.
// A reader would get wrong what a refusal writes: nothing, even when the bad item is the last of many good ones.

mod support;

use song_edit::Fs;
use support::Project;

#[test]
fn a_paste_lands_on_each_lane_and_plays() {
    let mut p = Project::open("mute-solo");
    let before_kicks = p.starts("kicks").len();
    let line = r#"events add [{"track":"kicks","ref":"kick","start":"3:1"},{"track":"snares","ref":"snare","start":"3:3","fadeIn":"0:0:1","gain":2},{"track":"kicks","ref":"kick","start":"4:1","to":0.05,"loop":2}]"#;
    let ran = p.round_trip(line);
    assert_eq!(ran.answer.log, ["added 3 events on kicks, snares"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    p.ok(line);
    let kicks = p.starts("kicks");
    assert_eq!(kicks.len(), before_kicks + 2);
    assert!(kicks.contains(&8.0) && kicks.contains(&12.0), "{kicks:?}");
    let snare = p
        .track("snares")
        .clips
        .into_iter()
        .find(|c| (c.start - 10.0).abs() < 1e-6)
        .expect("the pasted snare");
    assert!(snare.fade_in > 0.0);
    // Only the listed fields are written: a pasted `gain` is not one of them.
    assert!(
        !p.text("nodes/snares.k7x")
            .contains("start(\"3:3\").fadeIn(\"0:0:1\").gain")
    );
    assert!(p.text("nodes/kicks.k7x").contains("start(\"4:1\").to(0.05).loop(2)"));
}

#[test]
fn a_start_in_seconds_stamps_the_format_it_needs() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip(r#"events add [{"track":"drums","ref":"kick","start":4.5,"fadeOut":"0:0:1"}]"#);
    assert_eq!(ran.changed(&p.dir), ["nodes/drums.k7x", "project.k7x"]);
    p.ok(r#"events add [{"track":"drums","ref":"kick","start":4.5,"fadeOut":"0:0:1"}]"#);
    assert!(p.text("project.k7x").contains("format(2)"), "{}", p.text("project.k7x"));
    assert!(
        p.text("nodes/drums.k7x").contains("start(4.5).fadeOut(\"0:0:1\")"),
        "{}",
        p.text("nodes/drums.k7x")
    );
    assert!(
        p.starts("drums").iter().any(|b| (b - 9.0).abs() < 1e-6),
        "4.5 s at 120 bpm is beat 9"
    );
}

#[test]
fn a_bad_item_anywhere_writes_nothing() {
    let mut p = Project::open("clip-lane");
    let ok = r#"{"track":"drums","ref":"kick","start":"3:1"}"#;
    let bad = |item: &str| format!("events add [{ok},{item}]");
    p.refused("events add", "usage: events add <JSON");
    p.refused("events add [", "bad events JSON");
    p.refused("events add []", "must be a non-empty JSON array");
    p.refused(&bad("3"), "events add [1]: needs a string track and ref");
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":-1}"#),
        "events add [1]: needs a string track",
    );
    p.refused(
        &bad(r#"{"track":"bass","ref":"kick","start":"3:1"}"#),
        "events add [1]: no track \"bass\"",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3::1"}"#),
        "events add [1]: bad timecode",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"hat","start":"3:1"}"#),
        "no clip, sequence or timeline \"hat\"",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","loop":0}"#),
        "events add [1]: loop — ",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","loop":1.5}"#),
        "events add [1]: loop — ",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","warp":"yes"}"#),
        "events add [1]: warp — ",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","disabled":null}"#),
        "disabled — expected a value",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","from":"x"}"#),
        "events add [1]: from — ",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","from":2,"to":1}"#),
        "to — 'to' must be greater than 'from'",
    );
    p.refused(
        &bad(r#"{"track":"drums","ref":"kick","start":"3:1","fadeIn":{"over":1,"by":2}}"#),
        "events add [1]: fadeIn — ",
    );
}

#[test]
fn a_timeline_event_takes_no_extent_of_its_own() {
    let mut p = Project::open("clip-lane");
    let tl = format!("{}/timelines/fill.k7x", p.dir);
    p.fs.mkdir_all(&format!("{}/timelines", p.dir)).expect("a folder");
    p.fs.write_atomic(
        &tl,
        b"length(\"1:0\")\nlanes([lane(\"a\").events([ref(\"kick\").start(\"1:1\")])])\n",
    )
    .expect("a write");
    p.refused(
        r#"events add [{"track":"drums","ref":"fill","start":"3:1","loop":2}]"#,
        "timeline 'fill' can't take loop — its length is fixed",
    );
    p.round_trip(r#"events add [{"track":"drums","ref":"fill","start":"3:1"}]"#);
}
