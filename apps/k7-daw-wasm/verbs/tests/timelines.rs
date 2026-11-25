// timelines.rs — the timeline verbs on real fixtures: new, length, lanes, rename, copy, a selection made one, and play's ask.
// A reader would get wrong a timeline's sound: it plays only where an event places it, so the doc is read through `event add`.

mod support;

use support::Project;

fn lanes(p: &Project, id: &str) -> Vec<String> {
    match p.data(&format!("timelines/{id}.k7x")).get("lanes") {
        Some(k7x::edit::Value::List(l)) => l
            .iter()
            .filter_map(|x| match x.get("lane") {
                Some(k7x::edit::Value::Str(s)) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn a_new_timeline_is_four_bars_and_one_lane() {
    let mut p = Project::open("clip-lane");
    let ran = p.run("timeline new verse");
    assert!(!ran.answer.refused, "{:?}", ran.answer.refuse);
    assert_eq!(ran.changed(&p.dir), ["timelines/verse.k7x"]);
    assert_eq!(
        ran.answer.log,
        ["+ timeline verse (4:0) — place it with: event add <track> verse <timecode>"]
    );
    let tl = p.doc().timelines.unwrap_or_default();
    assert_eq!(tl.len(), 1);
    assert!((tl[0].length - 16.0).abs() < 1e-9);
    p.undo(&ran);
    assert!(!p.exists("timelines/verse.k7x"));
    p.ok("timeline new");
    assert!(p.exists("timelines/timeline-1.k7x"));
    p.ok("timeline new verse");
    p.refused("timeline new verse", "timeline 'verse' already exists");
    p.refused("timeline new main", "invalid timeline name 'main'");
    p.refused("timeline new chorus 0:0", "'0:0' is not a duration");
}

#[test]
fn a_timeline_length_and_its_lanes() {
    let mut p = Project::open("clip-lane");
    p.ok("timeline new verse");
    p.round_trip("timeline length verse 8:0");
    p.ok("timeline length verse 8:0");
    let tl = p.doc().timelines.unwrap_or_default();
    assert!((tl[0].length - 32.0).abs() < 1e-9);
    p.round_trip("timeline lane new verse");
    p.ok("timeline lane new verse");
    p.ok("timeline lane new verse bass");
    assert_eq!(lanes(&p, "verse"), ["lane-1", "lane-2", "bass"]);
    p.refused("timeline lane new verse bass", "already has a lane 'bass'");
    p.round_trip("timeline lane remove verse lane-2");
    p.ok("timeline lane remove verse lane-2");
    p.ok("timeline lane remove verse bass");
    assert_eq!(lanes(&p, "verse"), ["lane-1"]);
    p.refused("timeline lane remove verse lane-1", "is the last lane of 'verse'");
    p.refused("timeline lane remove verse nope", "timeline 'verse' has no lane 'nope'");
    p.refused("timeline length ghost 2:0", "no timeline \"ghost\"");
}

#[test]
fn a_selection_becomes_a_timeline_placed_in_its_stead() {
    let mut p = Project::open("clip-lane");
    let ran = p.round_trip("selection timeline drums 1:1 1:2 hits");
    assert_eq!(
        ran.answer.log,
        ["+ timeline hits: 2 events of drums, placed at 1:1 in their stead"]
    );
    p.ok("selection timeline drums 1:1 1:2 hits");
    let drums = p.track("drums");
    assert!(
        drums
            .clips
            .iter()
            .any(|c| c.nest.as_ref().is_some_and(|n| n.ref_ == "hits"))
    );
    assert_eq!(p.doc().timelines.unwrap_or_default().len(), 1);
    p.refused("selection timeline drums,drums2 1:1 1:2", "takes ONE track");
    p.refused("selection timeline drums 2:1 1:1", "is before the start");
    p.refused(
        "selection timeline drums 30:1 31:1 empty",
        "nothing on 'drums' starts between",
    );

    let ran = p.ok("timeline play hits");
    assert_eq!(
        ran.answer.asks,
        [serde_json::json!({ "route": "POST /verb-host/engine", "args": { "op": "timelinePlay", "id": "hits" } })]
    );
    assert_eq!(ran.answer.log, ["playing timeline hits alone, through drums"]);
    let ran = p.ok("timeline play");
    assert_eq!(
        ran.answer.asks,
        [serde_json::json!({ "route": "POST /verb-host/engine", "args": { "op": "timelinePlay", "id": null } })]
    );
    p.refused("timeline play ghost", "no timeline \"ghost\"");

    let ran = p.round_trip("timeline rename hits stabs");
    assert_eq!(ran.answer.log, ["hits → stabs, and the 1 event placing it"]);
    p.ok("timeline rename hits stabs");
    assert!(
        p.track("drums")
            .clips
            .iter()
            .any(|c| c.nest.as_ref().is_some_and(|n| n.ref_ == "stabs"))
    );
    p.refused("timeline rename stabs main", "invalid timeline name 'main'");
}

#[test]
fn a_lane_is_copied_into_a_timeline_and_back_to_the_song() {
    let mut p = Project::open("clip-lane");
    p.ok("selection timeline drums 1:1 1:2 hits");
    p.ok("timeline new verse");
    let ran = p.round_trip("timeline lane copy hits>drums verse");
    assert_eq!(ran.answer.log, ["verse: + lane drums, 2 event(s) of hits>drums"]);
    p.ok("timeline lane copy hits>drums verse");
    p.ok("timeline lane copy hits>drums verse");
    assert_eq!(lanes(&p, "verse"), ["lane-1", "drums", "drums-2"]);
    let ran = p.round_trip("timeline lane copy hits>drums main hits-lane");
    assert!(
        ran.answer.log[0].starts_with("+ track hits-lane: 2 event(s) of hits>drums"),
        "{:?}",
        ran.answer.log
    );
    p.ok("timeline lane copy hits>drums main hits-lane");
    assert_eq!(p.starts("hits-lane"), [0.0, 1.0]);
    p.refused("timeline lane copy drums main", "is already on the main timeline");
    p.refused("timeline lane copy drums ghost", "no timeline \"ghost\"");
    p.refused(
        "timeline lane copy drums hits",
        "places timeline 'hits', which would then play inside 'hits'",
    );
    p.refused("timeline lane copy nobody verse", "no track \"nobody\"");
}
