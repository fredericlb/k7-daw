// graph_model.rs — what a graph is made of: lanes, placements, chains and modules, as `buildGraph` and `loadGraph` make them.
// A reader would get wrong that these cases lay files and load them: the TypeScript's hand-built documents have no Rust twin.

use crate::graph_support;

use graph_support::{OUT_AMP, lane, refused, with_graph};
use song::graph::{ClipKind, TrackType};

// graph.test.ts "drops a disabled lane and the chain sourced from it, keeping the rest" (412: disabled is sorted).
#[test]
fn a_disabled_lane_and_the_chain_it_feeds_are_dropped_and_the_rest_kept() {
    let files = [
        ("nodes/melody.k7x", lane("").as_str().to_owned()),
        ("nodes/pad.k7x", lane("disabled(true)\n")),
        (
            "graph.k7x",
            "modules({ melody: [\"OUT\"], pad: [\"OUT\"] })\n".to_owned(),
        ),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    with_graph(&files, |g| {
        assert_eq!(g.tracks.keys().copied().collect::<Vec<_>>(), ["melody"]);
        assert_eq!(g.disabled, ["pad"]);
        assert_eq!(g.chains.keys().collect::<Vec<_>>(), ["melody"]);
    });
}

// graph.test.ts "keeps a BUS chain alive when one of its sources is disabled".
#[test]
fn a_bus_chain_lives_when_one_source_is_disabled() {
    let melody = lane("");
    let pad = lane("disabled(true)\n");
    let files = [
        ("nodes/melody.k7x", melody.as_str()),
        ("nodes/pad.k7x", pad.as_str()),
        ("nodes/reverb.k7x", OUT_AMP),
        (
            "graph.k7x",
            "modules({ melody: [\"reverb\"], pad: [\"reverb\"], reverb: [\"OUT\"] })\n",
        ),
    ];
    with_graph(&files, |g| {
        let mut names: Vec<_> = g.chains.keys().cloned().collect();
        names.sort();
        assert_eq!(names, ["melody", "reverb"]);
    });
}

// graph.test.ts "keeps warning about a node that names nothing at all": the chain is drawn to be seen.
#[test]
fn a_node_that_names_nothing_is_still_a_chain() {
    with_graph(&[("graph.k7x", "modules({ ghost: [\"OUT\"] })\n")], |g| {
        assert_eq!(g.chains.keys().collect::<Vec<_>>(), ["ghost"]);
        assert!(g.modules.get("ghost").is_none());
    });
}

// 413 and graph.test.ts "carries an automation's trigger, and adds none when the file states none".
#[test]
fn a_module_carries_its_automations_with_their_triggers() {
    let amp = format!(
        "{OUT_AMP}automations([{{ param: \"cut\", curve: \"@meta/curves/a\", trigger: \"kick\" }}, {{ param: \"res\", curve: \"b\" }}])\n"
    );
    with_graph(&[("nodes/amp.k7x", amp.as_str())], |g| {
        assert_eq!(
            g.modules["amp"].automations,
            [("cut", "a", Some("kick")), ("res", "b", None)]
        );
    });
}

// graph.test.ts "carries the module's named base": the module is borrowed, so the view reads `base` off it.
#[test]
fn a_module_borrows_its_resolved_module_and_its_base() {
    let amp = format!("{OUT_AMP}base(\"warm\")\n");
    with_graph(&[("nodes/amp.k7x", amp.as_str())], |g| {
        assert_eq!(g.modules["amp"].m.authored.base.as_deref(), Some("warm"));
    });
}

// 417: a placement is audio, midi, timeline or missing, a clip winning over a sequence and a sequence over a timeline.
#[test]
fn a_placement_names_a_clip_then_a_sequence_then_a_timeline_else_nothing() {
    let l = lane(
        "events([ref(\"kick\").start(\"2:1\"), ref(\"riff\").start(\"1:1\"), ref(\"song\").start(\"3:1\"), ref(\"nope\").start(\"1:1\")])\n",
    );
    let files = [
        ("clips/kick.k7x", "source(\"clips/kick.wav\")\n"),
        ("sequences/riff.k7x", "notes([t(0).d(1).k(60)])\n"),
        ("timelines/song.k7x", "lanes({ l: [] })\n"),
        ("nodes/a.k7x", l.as_str()),
    ];
    with_graph(&files, |g| {
        let kinds: Vec<_> = g.tracks["a"].placements.iter().map(|p| (p.ref_, p.kind)).collect();
        assert_eq!(kinds[0], ("kick", ClipKind::Audio));
        assert_eq!(kinds[1], ("riff", ClipKind::Midi));
        assert_eq!(kinds[3], ("nope", ClipKind::Missing));
        assert_eq!(g.tracks["a"].kind, TrackType::Single);
        // 2:1 in 4/4 is four beats in.
        assert!((g.tracks["a"].placements[0].start - 4.0).abs() < 1e-9);
    });
}

// 402: `@meta/track` as a graph step is a LANE here and not in the compile (244).
#[test]
fn a_step_naming_a_vocabulary_track_is_a_lane_of_the_graph_and_not_of_the_compile() {
    let files = [("graph.k7x", "modules({ \"@meta/track\": [\"OUT\"] })\n")];
    with_graph(&files, |g| {
        let c = &g.chains["@meta/track"];
        assert_eq!(c.from.as_deref(), Some("@meta/track"));
        assert!(c.use_steps.is_empty());
    });
}

// 403: picture lanes are apart from the sound lanes, and a bypass mutes either.
#[test]
fn picture_lanes_are_apart_and_a_bypass_mutes() {
    let files = [
        (
            "nodes/pic.k7x",
            "extends(\"@meta/picture-track\")\ncolor(\"#ff0000\")\n",
        ),
        ("nodes/snd.k7x", "extends(\"@meta/track\")\n"),
        (
            "graph.k7x",
            "modules({ pic: [\"RENDER\"], snd: [\"OUT\"] })\nbypassed([\"snd\", \"pic\"])\n",
        ),
    ];
    with_graph(&files, |g| {
        assert_eq!(g.video_tracks.keys().copied().collect::<Vec<_>>(), ["pic"]);
        assert!(g.video_tracks["pic"].mute);
        assert_eq!(g.video_tracks["pic"].color, Some("#ff0000"));
        assert_eq!(g.tracks.keys().copied().collect::<Vec<_>>(), ["snd"]);
        assert!(g.tracks["snd"].mute);
    });
}

// 405: a broken node refuses the graph, naming its file; a broken timeline does not (406).
#[test]
fn a_node_that_will_not_read_refuses_and_a_broken_timeline_does_not() {
    let r = refused(&[("nodes/bad.k7x", "plugin(\n")]);
    assert!(r.findings.iter().any(|f| f.file == "nodes/bad.k7x"));
    let a = lane("events([ref(\"song\").start(\"1:1\")])\n");
    with_graph(
        &[("timelines/song.k7x", "lanes(\n"), ("nodes/a.k7x", a.as_str())],
        |g| {
            assert_eq!(g.tracks["a"].placements[0].kind, ClipKind::Missing);
            assert!(g.findings.iter().any(|f| f.file == "timelines/song.k7x"));
        },
    );
}

// 404: the wiring is checked over the RAW ports, and a fault refuses the graph.
#[test]
fn a_module_fed_too_few_inputs_refuses_the_graph() {
    let two = "plugin({ type: \"internal\", name: \"X\" })\ninputs([{ name: \"a\", type: \"audio\" }, { name: \"b\", type: \"audio\" }])\n";
    let files = [
        ("nodes/x.k7x", two),
        ("nodes/src.k7x", "extends(\"@meta/track\")\n"),
        ("graph.k7x", "modules({ src: [\"x\"], x: [\"OUT\"] })\n"),
    ];
    let r = refused(&files);
    assert!(r.faults.iter().any(|f| f.key().starts_with("too_few|chain:src:1|")));
}

// 413: positions ride through when any survive, and none otherwise.
#[test]
fn positions_are_there_only_when_the_file_has_some() {
    with_graph(&[("graph.k7x", "modules({ a: [\"OUT\"] })\n")], |g| {
        assert!(g.positions.is_none())
    });
    let files = [(
        "graph.k7x",
        "modules({ a: [\"OUT\"] })\npositions({ a: { x: 1, y: 2 } })\n",
    )];
    with_graph(&files, |g| {
        assert_eq!(g.positions.map(indexmap::IndexMap::len), Some(1))
    });
}
