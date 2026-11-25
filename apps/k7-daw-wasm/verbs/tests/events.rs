// events.rs — the event verbs of M1 on real fixtures: the lane file they write, the doc the engine then plays, and undo.
// A reader would get wrong the beats: 4/4 at 120 bpm, so bar 1 beat 3 is beat 2.0 and bar 3 is beat 8.0.

mod support;

use support::Project;

#[test]
fn a_move_within_a_lane_reaches_the_engine_and_undoes() {
    let mut p = Project::open("clip-lane");
    assert_eq!(p.starts("drums"), [0.0, 1.0, 2.0, 3.0, 4.0]);
    let ran = p.ok("event move drums 1:3:1 3:1:1");
    assert_eq!(ran.answer.log, ["drums: moved kick 1:3:1 → 3:1:1"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/drums.k7x"]);
    assert!(p.text("nodes/drums.k7x").contains("ref(\"kick\").start(\"3:1:1\")"));
    assert!(
        p.text("nodes/drums.k7x").contains("// One audio lane"),
        "the comment survives"
    );
    let mut starts = p.starts("drums");
    starts.sort_by(f64::total_cmp);
    assert_eq!(starts, [0.0, 1.0, 3.0, 4.0, 8.0]);
    p.undo(&ran);
    p.round_trip("event move drums 1:3:1 3:1:1");
}

#[test]
fn a_move_to_another_lane_carries_the_event() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("event move kicks 1:1:1 2:1:1 snares");
    p.ok("event move kicks 1:1:1 2:1:1 snares");
    assert_eq!(ran.changed(&p.dir), ["nodes/kicks.k7x", "nodes/snares.k7x"]);
    assert!(p.starts("kicks").is_empty());
    assert_eq!(p.starts("snares"), [1.0, 4.0]);
    p.refused("event move snares 1:2:1 1:2:1 nowhere", "no track \"nowhere\"");
}

#[test]
fn a_move_refuses_what_it_cannot_find() {
    let mut p = Project::open("clip-lane");
    p.refused("event move drums 9:1:1 2:1", "no event on 'drums' at 9:1:1");
    p.refused("event move nope 1:1 2:1", "no track \"nope\" — try: drums");
    p.refused("event move drums 1:1", "usage: event move");
    p.refused(
        "event move drums x 2:1",
        "<timecode> expects a bar:beat:sixteenth position",
    );
}

#[test]
fn a_bulk_move_shifts_every_lane_in_one_command() {
    let mut p = Project::open("group-lane");
    let ran = p.round_trip("events move kicks,snares 1:1 1:4 +1:0");
    assert_eq!(ran.answer.log, ["moved 4 events on kicks, snares by +1:0"]);
    p.ok("events move all 1:1 1:4 +1:0");
    assert_eq!(p.starts("kicks"), [4.0, 6.0]);
    assert_eq!(p.starts("snares"), [5.0, 7.0]);
    p.ok("events move all 2:1 2:4 -0:2");
    assert_eq!(p.starts("kicks"), [2.0, 4.0]);
    p.refused("events move kicks 1:1 4:4 -4:0", "would push an event before 1:1:1");
    p.refused(
        "events move kicks 9:1 9:4 +1:0",
        "no events between 9:1 and 9:4 on kicks",
    );
    p.refused("events move kicks 1:1 4:4 +0:0", "the delta is zero");
    p.refused("events move drums 1:1 4:4 +1:0", "no track \"drums\"");
    p.refused("events move kicks 1:1 1:4 1:0", "usage: events move");
}

#[test]
fn an_added_event_plays_and_a_missing_clip_is_refused() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("event add drums snare 3:1");
    assert_eq!(ran.answer.log, ["drums: added snare at 3:1"]);
    p.ok("event add drums snare 3:1");
    let drums = p.track("drums");
    let added = drums.clips.iter().find(|c| c.start == 8.0).expect("the event at bar 3");
    assert_eq!(added.name, "snare");
    p.refused("event add drums ghost 3:1", "no clip \"ghost\"");
    p.refused("event add drums kick 3:x", "expects a bar:beat:sixteenth position");
}

#[test]
fn a_removed_event_stops_playing_its_clip_stays() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("event remove drums 1:2:1");
    assert_eq!(ran.answer.log, ["drums: removed snare at 1:2:1"]);
    p.ok("event remove drums 1:2:1");
    assert_eq!(p.starts("drums"), [0.0, 2.0, 3.0, 4.0]);
    assert!(p.exists("clips/snare.k7x"));
    p.refused("event remove drums 1:2:1", "no event on 'drums' at 1:2:1");
}

#[test]
fn a_bulk_remove_takes_every_named_event_or_none() {
    let mut p = Project::open("clip-lane");
    let line = r#"events remove [{"track":"drums","start":"1:1:1"},{"track":"drums","start":"1:3:1"}]"#;
    let ran = p.round_trip(line);
    assert_eq!(ran.answer.log, ["removed 2 events from drums"]);
    p.ok(line);
    assert_eq!(p.starts("drums"), [1.0, 3.0, 4.0]);
    p.refused(
        r#"events remove [{"track":"drums","start":"1:2:1"},{"track":"drums","start":"8:1"}]"#,
        "no event on 'drums' at 8:1",
    );
    p.refused("events remove []", "usage: events remove");
}

#[test]
fn a_resize_sets_the_event_window_not_the_clip() {
    let mut p = Project::open("clip-fades");
    let clip = p.text("clips/kick.k7x");
    let ran = p.round_trip("event resize drums 1:3:1 1:3:1 1:3:2");
    assert_eq!(ran.answer.log[0], "drums: kick -> 1:3:1..1:3:2 (0:0:1)");
    p.ok("event resize drums 1:3:1 1:3:1 1:3:2");
    assert_eq!(p.text("clips/kick.k7x"), clip, "the clip keeps its own window");
    let kick = p
        .track("drums")
        .clips
        .into_iter()
        .find(|c| c.start == 2.0)
        .expect("the kick");
    assert!(
        kick.length <= 0.5 + 1e-9,
        "the event plays at most half a beat: {}",
        kick.length
    );
    p.refused("event resize drums 1:3:1 1:4 1:3", "must be after the start");
    p.refused("event resize drums 7:1 7:1 8:1", "no event on 'drums' at 7:1");
}

#[test]
fn an_audio_split_makes_two_windows_of_one_file() {
    let mut p = Project::open("clip-fades");
    let ran = p.round_trip("event split drums 1:3:1 1:3:1.100");
    assert_eq!(
        ran.answer.log[1],
        "  the source is cut at 0.0521s — nothing was rendered"
    );
    p.ok("event split drums 1:3:1 1:3:1.100");
    let pieces: Vec<_> = p.track("drums").clips.into_iter().filter(|c| c.start >= 2.0).collect();
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].source, pieces[1].source);
    assert!((pieces[1].start - (2.0 + 100.0 / 960.0)).abs() < 1e-6);
    p.refused(
        "event split drums 1:1:1 1:1:1",
        "the cut has to be after the event starts",
    );
    p.refused(
        "event split drums 1:3:1 1:3:3",
        "the cut has to be before the event ends",
    );
}

#[test]
fn a_sequence_split_writes_two_sequences_and_keeps_the_first() {
    let mut p = Project::open("edit-undo");
    let ran = p.round_trip("event split melody 1:1:1 1:3:1");
    assert_eq!(
        ran.changed(&p.dir),
        ["nodes/melody.k7x", "sequences/melody-1.k7x", "sequences/melody-2.k7x"]
    );
    p.ok("event split melody 1:1:1 1:3:1");
    assert!(p.exists("sequences/melody.k7x"));
    let melody = p.track("melody");
    let notes: Vec<usize> = melody.clips.iter().map(|c| c.notes.len()).collect();
    assert_eq!(notes, [2, 2]);
    assert_eq!(melody.clips.iter().map(|c| c.start).collect::<Vec<_>>(), [0.0, 2.0]);
}
