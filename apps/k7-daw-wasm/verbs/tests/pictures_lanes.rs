// pictures_lanes.rs — M6's lane verbs: a picture lane made, ordered, renamed and deleted, in the song and in a timeline.
// A reader would get wrong how many files a rename touches: the lane, its strip, the graph, `trackOrder`, a sounding lane and a timeline's lane.

mod support;

use song_edit::Fs;
use support::{Project, strs};

fn write(p: &mut Project, rel: &str, text: &str) {
    let path = format!("{}/{rel}", p.dir);
    p.fs.write_atomic(&path, text.as_bytes()).expect("a write");
}

#[test]
fn a_new_lane_has_its_strip_and_draws_in_front() {
    let mut p = Project::open("video-lane");
    let ran = p.round_trip("picture lane new broll");
    assert_eq!(
        ran.changed(&p.dir),
        ["graph.k7x", "nodes/broll.k7x", "nodes/mix-broll.k7x", "project.k7x"]
    );
    p.ok("picture lane new broll");
    assert_eq!(
        strs(p.data("project.k7x").get("trackOrder")),
        ["drums", "broll", "main"]
    );
    let ran = p.ok("picture lane new");
    assert_eq!(ran.answer.log.len(), 1);
    assert!(
        ran.answer.log[0].starts_with("+ picture lane layer-1 (nodes/layer-1.k7x"),
        "{:?}",
        ran.answer.log
    );
    p.refused("picture lane new broll", "picture lane new:");
    p.refused("picture lane new bad/name", "invalid track name 'bad/name'");
}

#[test]
fn a_timeline_gets_a_picture_lane_of_its_own() {
    let mut p = Project::open("video-lane");
    p.ok("timeline new verse");
    let ran = p.round_trip("picture lane new verse>cam");
    assert_eq!(ran.answer.log, ["verse: + picture lane cam"]);
    p.ok("picture lane new verse>cam");
    p.refused(
        "picture lane new verse>cam",
        "timeline 'verse' already has a lane 'cam'",
    );
    p.refused("picture lane new nope>cam", "no timeline 'nope'");
    p.refused("picture lane new verse>bad/x", "invalid lane name 'bad/x'");
    let ran = p.round_trip("picture lane rename verse>cam wide");
    assert_eq!(
        ran.answer.log,
        ["verse>cam → verse>wide — it now draws on the song's picture lane 'wide'"]
    );
    p.refused(
        "picture lane rename verse>cam cam",
        "'cam' cannot name a lane of 'verse'",
    );
    p.ok("picture lane rename verse>cam wide");
    let ran = p.round_trip("picture lane delete verse>wide");
    assert_eq!(ran.answer.log, ["verse: - picture lane wide"]);
    p.ok("picture lane delete verse>wide");
    p.refused(
        "picture lane delete verse>wide",
        "timeline 'verse' has no picture lane 'wide'",
    );
}

#[test]
fn the_draw_order_is_the_picture_rows_with_the_sound_rows_left_where_they_are() {
    let mut p = Project::open("video-lane");
    p.ok("picture lane new broll");
    p.ok("picture lane new top");
    assert_eq!(
        strs(p.data("project.k7x").get("trackOrder")),
        ["drums", "top", "broll", "main"]
    );
    let ran = p.round_trip("picture lane order main 3");
    assert_eq!(ran.answer.log, ["main is now layer 3 of 3, bottom first (project.k7x)"]);
    p.ok("picture lane order main 3");
    assert_eq!(
        strs(p.data("project.k7x").get("trackOrder")),
        ["drums", "main", "top", "broll"]
    );
    p.ok("picture lane order broll top");
    p.refused(
        "picture lane order ghost 1",
        "no picture lane 'ghost' — this song's are",
    );
    p.refused(
        "picture lane order main sideways",
        "neither a position nor a lane of this song",
    );
    p.refused("picture lane order main", "usage: picture lane order");
}

#[test]
fn a_rename_follows_every_place_that_names_the_lane() {
    let mut p = Project::open("video-lane");
    p.ok("picture lane new broll");
    p.ok("picture solo broll");
    write(
        &mut p,
        "nodes/dialog.k7x",
        "extends(\"@meta/track\")\nvideo_source(\"broll\")\n",
    );
    p.ok("timeline new verse");
    p.ok("picture lane new verse>broll");
    let ran = p.round_trip("picture lane rename broll cutaway");
    assert_eq!(ran.answer.log.len(), 1);
    let log = &ran.answer.log[0];
    assert!(log.starts_with("broll → cutaway ("), "{log}");
    assert!(
        log.contains("mix-broll → mix-cutaway") && log.contains("dialog's sound") && log.contains("verse>cutaway"),
        "{log}"
    );
    p.ok("picture lane rename broll cutaway");
    let root = p.data("project.k7x");
    assert_eq!(strs(root.get("trackOrder")), ["drums", "cutaway", "main"]);
    assert_eq!(strs(root.get("soloed")), ["cutaway"]);
    assert!(p.text("graph.k7x").contains("cutaway") && !p.text("graph.k7x").contains("broll"));
    assert!(p.text("nodes/dialog.k7x").contains("video_source(\"cutaway\")"));
    assert!(p.exists("nodes/cutaway.k7x") && !p.exists("nodes/broll.k7x") && p.exists("nodes/mix-cutaway.k7x"));
    assert_eq!(
        p.data("timelines/verse.k7x")
            .get("pictures")
            .map(|v| format!("{v:?}").contains("cutaway")),
        Some(true)
    );
}

#[test]
fn a_rename_that_cannot_be_clean_is_refused_with_nothing_written() {
    let mut p = Project::open("video-lane");
    p.ok("picture lane new broll");
    p.refused("picture lane rename broll broll", "'broll' is already called that");
    p.refused(
        "picture lane rename broll drums",
        "'drums' is already a track or a module in this project",
    );
    p.refused("picture lane rename broll a/b", "invalid lane name 'a/b'");
    p.refused("picture lane rename ghost x", "no picture lane 'ghost'");
    p.refused("picture lane rename broll", "usage: picture lane rename");
    p.refused("picture lane rename drums x", "no picture lane 'drums'");
}

#[test]
fn a_delete_takes_the_lane_its_strip_and_its_rows_but_not_its_clips() {
    let mut p = Project::open("video-lane");
    p.ok("picture lane new broll");
    p.ok("picture solo broll");
    p.ok("timeline new verse");
    p.ok("picture lane new verse>broll");
    let ran = p.round_trip("picture lane delete broll");
    assert_eq!(ran.answer.log.len(), 2);
    assert!(
        ran.answer.log[0].starts_with("deleted picture lane broll (")
            && ran.answer.log[0].ends_with("— its clips stay on disk")
    );
    assert_eq!(
        ran.answer.log[1],
        "verse>broll now draw(s) nowhere — 'picture lane new broll' brings a lane back"
    );
    p.ok("picture lane delete broll");
    assert!(!p.exists("nodes/broll.k7x") && !p.exists("nodes/mix-broll.k7x") && p.exists("clips/sample.k7x"));
    let root = p.data("project.k7x");
    assert_eq!(strs(root.get("trackOrder")), ["drums", "main"]);
    assert!(root.get("soloed").is_none());
    assert!(!p.text("graph.k7x").contains("broll"));
}

#[test]
fn a_delete_waits_for_the_lane_that_plays_its_sound() {
    let mut p = Project::open("video-lane");
    write(
        &mut p,
        "nodes/dialog.k7x",
        "extends(\"@meta/track\")\nvideo_source(\"main\")\n",
    );
    p.refused(
        "picture lane delete main",
        "dialog plays 'main''s sound — delete it first (track delete dialog)",
    );
    p.refused("picture lane delete", "usage: picture lane delete <lane>");
}

#[test]
fn a_lane_the_sound_half_is_wired_to_is_dropped_whole() {
    let mut p = Project::open("video-lane");
    write(
        &mut p,
        "graph.k7x",
        "modules({ drums: [\"MIX\"], MIX: [\"OUT\"], \"main#audio\": [\"MIX\"], RENDER: [\"VIDEO_OUT\"] })\n",
    );
    p.round_trip("picture lane delete main");
    p.ok("picture lane delete main");
    assert!(!p.text("graph.k7x").contains("main"), "{}", p.text("graph.k7x"));
}

#[test]
fn a_chain_project_renames_and_deletes_a_picture_lane_with_its_chain() {
    let mut p = Project::open_crate("chain-lanes");
    p.ok("picture lane new vid");
    assert!(p.exists("tracks/vid.k7x") && p.exists("chains/vid.k7x"));
    p.round_trip("picture lane rename vid film");
    p.ok("picture lane rename vid film");
    assert!(p.exists("tracks/film.k7x") && !p.exists("tracks/vid.k7x"));
    assert!(!strs(p.data("project.k7x").get("trackOrder")).contains(&"vid".to_owned()));
    p.round_trip("picture lane delete film");
    p.ok("picture lane delete film");
    assert!(!p.exists("tracks/film.k7x") && !p.exists("chains/film.k7x"));
    assert!(!strs(p.data("project.k7x").get("trackOrder")).contains(&"film".to_owned()));
}
