// lanes_m4.rs — M4's lane verbs on real fixtures, graph and chain: group, hide, input, arrange and duplicate.
// A reader would get wrong a duplicate's chain: the copy's nodes are its OWN, so the doc shows two plugins, not one shared.

mod support;

use support::{Project, strs};

#[test]
fn a_group_takes_its_tracks_out_of_the_one_they_were_in() {
    let mut p = Project::open("group-lane");
    let ran = p.round_trip("track group kicks");
    assert_eq!(
        ran.answer.log,
        [
            "+ group group-1: kicks (nodes/group-1.k7x)",
            "  out of drums (nodes/drums.k7x)"
        ]
    );
    p.ok("track group kicks");
    assert_eq!(strs(p.data("nodes/drums.k7x").get("tracks")), ["snares"]);
    assert_eq!(
        p.text("nodes/group-1.k7x"),
        "extends(\"@meta/group-track\")\ntracks([\"kicks\"])\n"
    );
    assert_eq!(p.track("kicks").group, "group-1");
    p.refused("track group drums", "'drums' is a group");
    p.refused("track group ghost", "no track 'ghost'");
    p.refused("track group", "usage: track group");
}

#[test]
fn a_group_in_a_chain_project_has_its_own_chain() {
    let mut p = Project::open_crate("chain-lanes");
    let ran = p.round_trip("track group drums,keys band");
    assert_eq!(ran.changed(&p.dir), ["chains/band.k7x", "tracks/band.k7x"]);
    p.ok("track group drums,keys band");
    assert_eq!(p.track("drums").group, "band");
    assert_eq!(p.track("keys").group, "band");
}

#[test]
fn a_hidden_lane_leaves_the_song_and_its_solo() {
    let mut p = Project::open_crate("chain-lanes");
    p.ok("track solo drums");
    let ran = p.round_trip("track hide drums");
    assert_eq!(ran.changed(&p.dir), ["chains/drums.k7x", "project.k7x"]);
    assert_eq!(
        ran.answer.log,
        ["drums hidden (chains/drums.k7x, unsoloed in project.k7x) — `track unhide drums` brings it back"]
    );
    p.ok("track hide drums");
    assert!(p.doc().tracks.iter().all(|t| t.id != "drums"));
    assert!(p.text("chains/drums.k7x").contains("hidden(true)"));
    p.refused("track hide drums", "\"drums\" is already hidden");
    p.round_trip("track unhide drums");
    p.ok("track unhide drums");
    assert!(p.doc().tracks.iter().any(|t| t.id == "drums"));
    p.refused("track unhide drums", "\"drums\" is already shown");
    p.refused("track hide ghost", "no track \"ghost\" in this project");
    let mut g = Project::open("mute-solo");
    g.refused("track hide kicks", "only a chain project has chain files");
}

#[test]
fn an_input_feeds_a_lane_once() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track input kicks audio");
    assert_eq!(ran.answer.log, ["kicks: audio input kicks_audio_in_1 wired to it"]);
    p.ok("track input kicks audio");
    assert!(
        p.text("graph.k7x").contains("kicks_audio_in_1: [\"kicks\"]"),
        "{}",
        p.text("graph.k7x")
    );
    p.refused("track input kicks midi", "kicks already has an input");
    p.refused("track input ghost audio", "no track \"ghost\"");
    p.refused("track input kicks video", "usage: track input <track> audio|midi");

    let mut c = Project::open_crate("chain-lanes");
    let ran = c.round_trip("track input keys midi");
    assert_eq!(ran.changed(&c.dir), ["chains/keys.k7x"]);
    c.ok("track input keys midi");
    assert!(c.text("chains/keys.k7x").contains("@meta/MIDI"));
    c.refused("track input keys audio", "keys already has an input");
}

#[test]
fn arrange_colours_every_lane_and_states_the_order() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("track arrange");
    assert!(
        ran.answer.log[0].starts_with("arranged 3 tracks — "),
        "{:?}",
        ran.answer.log
    );
    assert!(ran.changed(&p.dir).contains(&"project.k7x".to_owned()));
    p.ok("track arrange");
    let order = strs(p.data("project.k7x").get("trackOrder"));
    assert_eq!(order.len(), 3);
    assert!(p.data("nodes/kicks.k7x").get("color").is_some());
    assert_eq!(p.doc().tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(), order);
}

#[test]
fn a_duplicate_copies_the_lane_its_events_and_its_strip() {
    let mut p = Project::open("mixer-strip");
    let ran = p.round_trip("track duplicate drums");
    assert_eq!(
        ran.answer.log,
        ["+ track drums-copy (nodes/drums-copy.k7x, nodes/mix-drums-copy.k7x, graph.k7x)"]
    );
    p.ok("track duplicate drums");
    assert_eq!(p.starts("drums-copy"), p.starts("drums"));
    assert_eq!(p.track("drums-copy").plugins.len(), 1);
    assert!(
        p.text("graph.k7x").contains("\"drums-copy\": [\"mix-drums-copy\"]")
            || p.text("graph.k7x").contains("drums-copy: [\"mix-drums-copy\"]")
    );
    p.ok("track duplicate empty drums");
    assert!(p.track("drums-copy-2").clips.is_empty());
    p.refused("track duplicate ghost", "no track \"ghost\"");
    let mut g = Project::open("group-lane");
    g.refused("track duplicate drums", "\"drums\" is a group");
}

#[test]
fn a_chain_duplicate_gets_fresh_ids() {
    let mut p = Project::open_crate("chain-lanes");
    let ran = p.round_trip("track duplicate drums");
    assert_eq!(ran.changed(&p.dir), ["chains/drums-copy.k7x", "tracks/drums-copy.k7x"]);
    p.ok("track duplicate drums");
    let chain = p.text("chains/drums-copy.k7x");
    assert!(chain.contains("track(\"drums-copy\")"), "{chain}");
    assert!(!chain.contains("0000000000d2"), "the copy shares no id: {chain}");
    assert_eq!(p.starts("drums-copy"), [0.0, 1.0]);
    assert_eq!(p.track("drums-copy").plugins.len(), 1);
}
