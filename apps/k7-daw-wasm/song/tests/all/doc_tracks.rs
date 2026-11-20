// doc_tracks.rs — the tracks of a document (decisions 306 to 311, 321 to 329): halves, content types, groups, buses, inputs, colours, the head.
// Ported from trackModule.test.ts, liveInput.test.ts, soloed.test.ts, tapOuts.test.ts, masterChain.test.ts and console.test.ts, over song::compile.

use crate::doc_support;

use doc_support::{AMP, compiled, compiled_with, doc_of, lane, project, stopped};
use serde_json::json;
use song::doc::{ProjectDoc, TrackDoc, Vsts};

fn track<'a>(d: &'a ProjectDoc, id: &str) -> &'a TrackDoc {
    d.tracks.iter().find(|t| t.id == id).unwrap_or_else(|| {
        panic!(
            "no track {id} among {:?}",
            d.tracks.iter().map(|t| &t.id).collect::<Vec<_>>()
        )
    })
}

fn ids(d: &ProjectDoc) -> Vec<&str> {
    d.tracks.iter().map(|t| t.id.as_str()).collect()
}

const SEQ: &str = "notes([t(0).d(1).k(60)])\n";
const CLIP: &str = "source(\"k.wav\")\n";

fn with_material(modules: &str, lanes: &[(&str, &str)], extra: &[(&str, &str)]) -> ProjectDoc {
    let mut files: Vec<(&str, &str)> = vec![
        ("sequences/s.k7x", SEQ),
        ("clips/k.k7x", CLIP),
        ("k.wav", "w"),
        ("nodes/amp.k7x", AMP),
        ("nodes/fx.k7x", AMP),
    ];
    files.extend_from_slice(lanes);
    files.extend_from_slice(extra);
    let (g, text) = doc_support::graph(modules);
    let text = text.clone();
    files.push((g, &text));
    doc_of(&files)
}

// 309, 310, ports.test.ts "compiles both decks into one module": a lane keyed on two ports is two tracks, each a content type.
#[test]
fn a_lane_keyed_on_two_ports_is_two_halves_with_their_content() {
    let lane_text = lane("events([{ ref: \"s\", start: \"1:1\" }, { ref: \"k\", start: \"1:1\" }])\n");
    let d = with_material(
        "\"gtr#midi\": [\"amp\"], \"gtr#audio\": [\"fx\"], amp: [\"MIX\"], fx: [\"MIX\"]",
        &[("nodes/gtr.k7x", &lane_text)],
        &[],
    );
    assert_eq!(ids(&d), ["gtr#midi", "gtr#audio"]);
    assert_eq!(track(&d, "gtr#midi").content_type, "notes");
    assert_eq!(track(&d, "gtr#audio").content_type, "audio");
    assert_eq!(track(&d, "gtr#midi").clips.len(), 1);
    assert_eq!(track(&d, "gtr#audio").clips.len(), 1);
    assert_eq!(track(&d, "gtr#midi").name, "gtr#midi");
}

// 310: a lane on no port takes the content its events have: notes, audio, or both.
#[test]
fn a_lane_on_no_port_reads_its_content_off_its_events() {
    let both = lane("events([{ ref: \"s\", start: \"1:1\" }, { ref: \"k\", start: \"2:1\" }])\n");
    let midi = lane("events([{ ref: \"s\", start: \"1:1\" }])\n");
    let none = lane("");
    let d = with_material(
        "a: [\"amp\"], b: [\"fx\"], c: [\"MIX\"], amp: [\"MIX\"], fx: [\"MIX\"]",
        &[("nodes/a.k7x", &both), ("nodes/b.k7x", &midi), ("nodes/c.k7x", &none)],
        &[],
    );
    assert_eq!(track(&d, "a").content_type, "audio notes");
    assert_eq!(track(&d, "b").content_type, "notes");
    assert_eq!(track(&d, "c").content_type, "audio");
}

// 311: an event whose material has no half is one Failure on the lane's file, once per (lane, ref).
#[test]
fn an_event_with_no_half_is_a_failure_once_per_ref() {
    let lane_text = lane(
        "events([{ ref: \"s\", start: \"1:1\" }, { ref: \"s\", start: \"2:1\" }, { ref: \"k\", start: \"3:1\" }])\n",
    );
    let c = compiled(&[
        ("sequences/s.k7x", SEQ),
        ("clips/k.k7x", CLIP),
        ("k.wav", "w"),
        ("nodes/gtr.k7x", &lane_text),
        ("nodes/amp.k7x", AMP),
        ("graph.k7x", "modules({ \"gtr#audio\": [\"amp\"], amp: [\"MIX\"] })\n"),
    ]);
    let f = c.engine.as_ref().expect_err("a failure");
    assert_eq!(f.diagnostics.iter().filter(|d| d.file == "nodes/gtr.k7x").count(), 1);
}

// soloed.test.ts, mixFolding.md: a group's id folds onto its members; `solo` is the lane's or its group's, `mute` the lane's bypass.
#[test]
fn a_group_lists_its_members_and_a_solo_goes_through_it() {
    let g = "extends(\"@meta/group-track\")\ntracks([\"gtr\"])\ncolor(\"#112233\")\n";
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/drums.k7x", &lane("")),
        ("nodes/band.k7x", g),
        (
            "graph.k7x",
            "modules({ gtr: [\"MIX\"], drums: [\"MIX\"] })\nbypassed([\"drums\"])\n",
        ),
        ("project.k7x", "soloed([\"band\"])\n"),
    ]);
    let d = c.engine.expect("a document");
    assert_eq!(d.groups.len(), 1);
    assert_eq!(
        (
            d.groups[0].id.as_str(),
            d.groups[0].color.as_str(),
            d.groups[0].solo,
            d.groups[0].mute
        ),
        ("band", "#112233", true, false)
    );
    let t = track(&d, "gtr");
    assert_eq!((t.group.as_str(), t.solo, t.mute), ("band", true, false));
    let t = track(&d, "drums");
    assert_eq!((t.group.as_str(), t.solo, t.mute), ("", false, true));
}

// 324, highlight.md: the LAST mixer's output stage goes up 10 dB to the stage's ceiling; no mixer, nothing, and the flag stays.
#[test]
fn a_highlighted_lane_raises_its_last_mixer_to_the_ceiling() {
    let mixer = "plugin({ type: \"internal\", name: \"Mixer\" })\n";
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/drums.k7x", &lane("")),
        ("nodes/m.k7x", mixer),
        (
            "graph.k7x",
            "modules({ gtr: [\"m\"], m: [\"MIX\"], drums: [\"MIX\"] })\n",
        ),
        ("project.k7x", "highlighted([\"gtr\", \"drums\"])\n"),
    ]);
    let d = c.engine.expect("a document");
    assert_eq!(track(&d, "gtr").highlight, Some(true));
    assert_eq!(track(&d, "drums").highlight, Some(true));
    assert_eq!(track(&d, "drums").plugins.len(), 0);
    let p = &track(&d, "gtr").plugins[0];
    assert!(
        p.params.get("@output-volume").is_none_or(|v| *v == 10.0 || *v == 12.0),
        "{:?}",
        p.params
    );
}

// trackKind.md, 325: `trackOrder` full and partial; a lane it forgot follows the ones it names.
#[test]
fn track_order_places_the_listed_lanes_first() {
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/drums.k7x", &lane("")),
        ("nodes/vocals.k7x", &lane("")),
        (
            "graph.k7x",
            "modules({ gtr: [\"MIX\"], drums: [\"MIX\"], vocals: [\"MIX\"] })\n",
        ),
        ("project.k7x", "trackOrder([\"vocals\", \"gtr\"])\n"),
    ]);
    assert_eq!(ids(&c.engine.expect("a document")), ["vocals", "gtr", "drums"]);
    let c = project("gtr: [\"MIX\"], drums: [\"MIX\"]", &[]);
    assert_eq!(ids(&c.engine.expect("a document")), ["drums", "gtr"]);
}

// 327: a lane's colour is stated, or its category's shade by name (both halves of a lane share it).
#[test]
fn a_colour_is_stated_or_the_categorys_shade_by_name() {
    let d = with_material(
        "\"gtr#midi\": [\"MIX\"], \"gtr#audio\": [\"MIX\"], drums: [\"MIX\"], keys: [\"MIX\"]",
        &[
            ("nodes/gtr.k7x", &lane("")),
            ("nodes/drums.k7x", &lane("color(\"#abcdef\")\n")),
            ("nodes/keys.k7x", &lane("")),
        ],
        &[],
    );
    assert_eq!(track(&d, "drums").color, "#abcdef");
    assert!(track(&d, "keys").color.starts_with('#') && track(&d, "keys").color != "#abcdef");
    assert!(track(&d, "gtr#midi").color.starts_with('#'));
}

// liveInput.test.ts: an input carries the interface channels its label resolves to, and lands on the lane it records into.
#[test]
fn a_live_input_lands_on_its_lane_with_the_channels_of_its_label() {
    let rig = json!({ "channels": { "in": [{ "ch": [1, 2], "label": "mic" }] } });
    let c = compiled_with(
        &[
            ("nodes/gtr.k7x", &lane("")),
            (
                "nodes/rec.k7x",
                "io(\"in\")\nchannel(\"mic\")\ngain(-6)\nmonitor(true)\n",
            ),
            ("graph.k7x", "modules({ rec: [\"gtr\"], gtr: [\"MIX\"] })\n"),
        ],
        &rig,
        Vsts::Scan,
    );
    let d = c.engine.expect("a document");
    let t = track(&d, "gtr");
    assert_eq!(
        (t.input_channels.clone(), t.monitor.as_str(), t.input_gain),
        (vec![1, 2], "on", -6.0)
    );
}

// 322 / docs/busOnly.md, cue.md: a bus is muted by bypass, bus-only, or a cue with no pair; a cued bus carries the pair.
#[test]
fn a_bus_is_an_effect_track_muted_three_ways() {
    let c = project(
        "gtr: [\"fx\"], drums: [\"verb\"], fx: [\"MIX\"], verb: [\"MIX\"]",
        &[(
            "graph.k7x",
            "modules({ gtr: [\"amp\"], drums: [\"amp\"], amp: [\"MIX\"] })\nbus_only([\"amp\"])\n",
        )],
    );
    let d = c.engine.expect("a document");
    let bus = track(&d, "amp");
    assert_eq!(
        (bus.role.as_str(), bus.mute, bus.content_type.as_str()),
        ("effect", true, "audio")
    );
    let rig = json!({ "channels": { "out": [{ "ch": [3, 4], "label": "cue" }] }, "cueOutput": "cue" });
    let cued = |rig: &serde_json::Value| {
        compiled_with(
            &[
                ("nodes/gtr.k7x", &lane("")),
                ("nodes/drums.k7x", &lane("")),
                ("nodes/amp.k7x", AMP),
                (
                    "graph.k7x",
                    "modules({ gtr: [\"amp\"], drums: [\"amp\"], amp: [\"CUE\"] })\n",
                ),
            ],
            rig,
            Vsts::Scan,
        )
        .engine
        .expect("a document")
    };
    let heard = cued(&rig);
    assert_eq!(
        (track(&heard, "amp").output_channels.clone(), track(&heard, "amp").mute),
        (vec![3, 4], false)
    );
    let silent = cued(&json!({}));
    assert!(track(&silent, "amp").output_channels.is_empty() && track(&silent, "amp").mute);
}

// The head: defaults, uuid when non-empty, the master chain, markers and settings come from the head; `recordDir` is the project's clips.
#[test]
fn the_head_of_the_document_is_the_projects_with_its_defaults() {
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/limit.k7x", AMP),
        (
            "graph.k7x",
            "modules({ gtr: [\"MIX\"], MIX: [\"limit\"], limit: [\"OUT\"] })\n",
        ),
        (
            "project.k7x",
            "project({ uuid: \"abc\", name: \"song\", tempo: 90, sampleRate: 44100, timeSignature: \"3/4\", masterVolume: -3 })\n",
        ),
    ]);
    let d = c.engine.expect("a document");
    assert_eq!(
        (d.uuid.as_deref(), d.name.as_str(), d.tempo, d.sample_rate),
        (Some("abc"), "song", 90.0, 44100)
    );
    assert_eq!(
        (d.time_sig_numerator, d.time_sig_denominator, d.master_volume),
        (3, 4, -3.0)
    );
    assert_eq!(
        d.master_plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["MIX:limit", "MIX:@record"]
    );
    assert_eq!(d.record_dir, "/p/ws/proj/clips");
    assert!(d.timelines.is_none());
    let plain = doc_of(&[
        ("nodes/gtr.k7x", &lane("")),
        ("graph.k7x", "modules({ gtr: [\"MIX\"] })\n"),
    ]);
    assert_eq!(
        (
            plain.uuid,
            plain.name.as_str(),
            plain.tempo,
            plain.sample_rate,
            plain.master_volume
        ),
        (None, "untitled", 120.0, 48000, 0.0)
    );
}

// 330, outputRecording.md: a timeline surface means no recorder; a graph that never reaches OUT has none either.
#[test]
fn the_recorder_is_placed_only_without_a_timeline_and_with_a_run_to_out() {
    let nodes = [("nodes/gtr.k7x", lane(""))];
    let at = |graph: &str, surface: Option<&str>| {
        let mut files: Vec<(&str, &str)> = vec![("nodes/gtr.k7x", &nodes[0].1), ("graph.k7x", graph)];
        if let Some(s) = surface {
            files.push(("surfaces/t.k7x", s));
        }
        doc_of(&files).master_plugins.len()
    };
    let out = "modules({ gtr: [\"MIX\"], MIX: [\"OUT\"] })\n";
    assert_eq!(at(out, None), 1);
    assert_eq!(at(out, Some("type(\"timeline\")\nlayout(\"split\")\n")), 0);
    assert_eq!(at("modules({ gtr: [\"MIX\"] })\n", None), 0);
}

// 305: a folder with no graph, modules or chains is not a project.
#[test]
fn a_folder_that_is_no_project_is_one_failure_on_project_k7x() {
    let c = compiled(&[("project.k7x", "project({ name: \"x\" })\n")]);
    assert_eq!(stopped(&c), ["project.k7x"]);
}
