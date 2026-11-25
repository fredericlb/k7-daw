// tracks.rs — the lane verbs on real fixtures: new, order, rename, delete, mute, solo and highlight, their files, doc and undo.
// A reader would get wrong mute: it is the lane's bypass in graph.k7x, so the doc's `mute` is what the engine is told.

mod support;

use support::{Project, strs};

#[test]
fn a_new_track_is_a_lane_with_its_strip() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track new bass");
    assert_eq!(
        ran.answer.log,
        ["+ track bass (nodes/bass.k7x, nodes/mix-bass.k7x, graph.k7x)"]
    );
    assert_eq!(
        ran.changed(&p.dir),
        ["graph.k7x", "nodes/bass.k7x", "nodes/mix-bass.k7x"]
    );
    p.ok("track new bass");
    let bass = p.track("bass");
    assert!(bass.clips.is_empty());
    assert!(
        bass.plugins.iter().any(|pl| pl.id.contains("mix-bass")),
        "the strip plays on the lane"
    );
    p.refused("track new bass", "track 'bass' already exists");
    p.refused("track new no/slash", "invalid track name");
    p.ok("track new");
    assert!(p.doc().tracks.iter().any(|t| t.id == "track-1"));
}

#[test]
fn an_order_is_stated_whole_in_project_k7x() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track order muted 1");
    assert_eq!(ran.changed(&p.dir), ["project.k7x"]);
    p.ok("track order muted 1");
    assert_eq!(
        strs(p.data("project.k7x").get("trackOrder"))
            .first()
            .map(String::as_str),
        Some("muted")
    );
    assert_eq!(p.doc().tracks.first().map(|t| t.id.as_str()), Some("muted"));
    p.ok("track order muted kicks");
    p.ok("track order kicks 99");
    assert_eq!(p.doc().tracks.last().map(|t| t.id.as_str()), Some("kicks"));
    p.refused("track order ghost 1", "no track \"ghost\"");
    p.refused("track order kicks sideways", "neither a position nor a lane");
}

#[test]
fn a_rename_moves_the_file_the_lists_and_the_strip() {
    let mut p = Project::open("mute-solo");
    p.ok("track new bass");
    p.ok("track order bass 1");
    let ran = p.round_trip("track rename bass bassline");
    assert_eq!(
        ran.answer.log,
        ["bass → bassline (nodes/bass.k7x → nodes/bassline.k7x, trackOrder in project.k7x, mix-bass → mix-bassline)"]
    );
    p.ok("track rename bass bassline");
    assert!(!p.exists("nodes/bass.k7x") && p.exists("nodes/bassline.k7x") && p.exists("nodes/mix-bassline.k7x"));
    assert!(p.text("nodes/mix-bassline.k7x").contains("bassline's channel strip"));
    p.ok("track rename kicks kick-lane");
    assert_eq!(strs(p.data("project.k7x").get("soloed")), ["kick-lane"]);
    let doc = p.doc();
    assert!(
        doc.tracks
            .iter()
            .any(|t| t.id == "kick-lane" && t.solo && t.clips.len() == 1)
    );
    p.refused("track rename snares muted", "is already a node of this project");
    p.refused("track rename ghost x", "nodes/ghost.k7x does not exist");
    p.refused("track rename muted muted", "is already called that");
    p.refused("track rename muted no/slash", "invalid track name");
}

#[test]
fn a_delete_takes_the_lane_and_its_strip_and_forgets_it_in_the_lists() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track delete snares");
    assert_eq!(
        ran.answer.log,
        ["deleted snares and the 0 node(s) of its chain — its clips stay on disk"]
    );
    p.ok("track delete snares");
    assert!(strs(p.data("project.k7x").get("highlighted")).is_empty());
    assert!(!p.doc().tracks.iter().any(|t| t.id == "snares"));
    assert!(p.exists("clips/snare.k7x"));
    p.ok("track new bass");
    let ran = p.ok("track delete bass");
    assert!(ran.changed(&p.dir).contains(&"nodes/mix-bass.k7x".to_owned()));
    assert!(!p.exists("nodes/mix-bass.k7x"));
    p.refused("track delete ghost", "no track \"ghost\"");
}

#[test]
fn mute_is_the_lane_bypass_the_engine_is_told() {
    let mut p = Project::open("mute-solo");
    assert!(p.track("muted").mute);
    let ran = p.round_trip("track mute kicks");
    assert_eq!(ran.answer.effects, ["applyProject"]);
    assert_eq!(ran.changed(&p.dir), ["graph.k7x"]);
    p.ok("track mute kicks");
    assert!(p.track("kicks").mute);
    p.ok("track mute muted");
    assert!(!p.track("muted").mute);
    p.refused("track mute ghost", "no track \"ghost\"");
}

#[test]
fn solo_is_project_state_and_unmutes_what_it_solos() {
    let mut p = Project::open("mute-solo");
    p.round_trip("track solo muted");
    let ran = p.ok("track solo muted");
    assert_eq!(ran.answer.log, ["muted: solo on — soloed: kicks, muted"]);
    let doc = p.doc();
    let muted = doc.tracks.iter().find(|t| t.id == "muted").expect("muted");
    assert!(muted.solo && !muted.mute);
    p.ok("track solo kicks");
    p.ok("track solo muted");
    assert!(
        p.data("project.k7x").get("soloed").is_none(),
        "the last solo off deletes the list"
    );
    assert!(p.doc().tracks.iter().all(|t| !t.solo));
    p.refused("track solo", "usage: solo <track|group>");
}

#[test]
fn a_group_solos_its_members() {
    let mut p = Project::open("group-lane");
    p.ok("track solo drums");
    let doc = p.doc();
    assert!(doc.groups.iter().any(|g| g.id == "drums" && g.solo));
    assert!(doc.tracks.iter().filter(|t| t.group == "drums").all(|t| t.solo));
}

#[test]
fn highlight_toggles_the_lane_in_its_list() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track highlight kicks");
    assert_eq!(ran.answer.log, ["kicks: highlight on — 10 dB up"]);
    p.ok("track highlight kicks");
    assert_eq!(strs(p.data("project.k7x").get("highlighted")), ["snares", "kicks"]);
    assert_eq!(p.track("kicks").highlight, Some(true));
    p.ok("track highlight snares");
    p.ok("track highlight kicks");
    assert!(p.data("project.k7x").get("highlighted").is_none());
    p.refused("track highlight ghost", "no track \"ghost\"");
}
