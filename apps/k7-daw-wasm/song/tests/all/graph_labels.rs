// graph_labels.rs — the sentences outside validate_issues, the resolved `ir`, the lane strip and a node's place (411, 416, 435, 436).
// A reader would get wrong that the input labels are a warning and not a graph issue: `issues` holds validate's alone (445).

use crate::graph_support;

use graph_support::{keys, lane, sentences, with_graph};

fn lane_a() -> String {
    lane("")
}

// checkInputLabels: a label the rig lacks is said once per input, the rig's labels listed, the words kept as the TypeScript has them.
#[test]
fn an_unknown_input_label_is_said_with_the_rigs_labels_or_none() {
    let a = lane_a();
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("nodes/mic.k7x", "io(\"in\")\nchannel(\"ghost\")\n"),
        ("nodes/ok.k7x", "io(\"in\")\nchannel(\"synth\")\n"),
        ("nodes/num.k7x", "io(\"in\")\nchannel([2])\n"),
        (
            "graph.k7x",
            "modules({ mic: [\"a\"], ok: [\"OUT\"], num: [\"OUT\"] })\n",
        ),
    ];
    let rig = serde_json::json!({"channels": {"in": [{"ch": [3, 4], "label": "synth"}, {"ch": [1], "label": "vox"}]}});
    let w = graph_support::with_graph_rig(&files, &rig, |g| g.warnings.clone());
    assert_eq!(
        w.last().map(String::as_str),
        Some("input 'mic': channel 'ghost' is not an input label in preferences.yaml preferences — have: synth, vox")
    );
    let bare = sentences(&files);
    assert!(bare.iter().any(|x| x.ends_with("none are defined")));
    // The label is not a graph issue: `issues` holds validate's alone (445).
    assert!(keys(&files).iter().all(|k| !k.contains("ghost")));
}

// 416: an impulse response warns only when it did not resolve.
#[test]
fn an_impulse_response_warns_only_when_it_did_not_resolve() {
    let hall = |ir: &str| format!("plugin({{ type: \"internal\", name: \"Hall\" }})\nparams({{ ir: \"{ir}\" }})\n");
    let bad = hall("room.wav");
    let good = hall("@k7-core/room.wav");
    let g = |t: &str| {
        keys(&[
            ("lib/k7-core/irs/room.wav", "x"),
            ("nodes/hall.k7x", t),
            ("graph.k7x", "modules({ hall: [\"OUT\"] })\n"),
        ])
    };
    assert!(
        g(&bad)
            .iter()
            .any(|k| k.starts_with("moduleAssetMissing|module=hall|param=ir")),
        "{:?}",
        g(&bad)
    );
    assert!(
        g(&good).iter().all(|k| !k.starts_with("moduleAssetMissing")),
        "{:?}",
        g(&good)
    );
}

// 436: a lane's strip is the last node of its first run that is the mixer; a picture lane's is the one extending the picture strip.
#[test]
fn a_lane_strip_is_the_last_of_its_kind_in_the_first_run() {
    let a = lane_a();
    let files = [
        (
            "meta/audio/mixer/module.k7x",
            "plugin({ type: \"internal\", name: \"Mixer\" })\n",
        ),
        ("meta/shaders/video-mixer/module.k7x", "type(\"shader\")\n"),
        ("nodes/a.k7x", a.as_str()),
        ("nodes/mix-a.k7x", "extends(\"@meta/audio/mixer\")\n"),
        ("nodes/mix-b.k7x", "extends(\"@meta/audio/mixer\")\n"),
        ("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n"),
        ("nodes/vm.k7x", "extends(\"@meta/shaders/video-mixer\")\n"),
        (
            "graph.k7x",
            "modules({ a: [\"mix-a\"], \"mix-a\": [\"mix-b\"], \"mix-b\": [\"OUT\"], pic: [\"vm\"], vm: [\"RENDER\"] })\n",
        ),
    ];
    with_graph(&files, |g| {
        let idx = song::graph::Index::new(g);
        assert_eq!(song::graph::lane_strip(g, &idx, "a"), Some("mix-b"));
        assert_eq!(song::graph::lane_strip(g, &idx, "pic"), Some("vm"));
        assert_eq!(song::graph::lane_strip(g, &idx, "ghost"), None);
        let picture = song::graph::picture_lanes(g);
        assert_eq!(
            song::graph::lane_strips_of(g, &idx, &picture)
                .into_iter()
                .collect::<Vec<_>>(),
            [("pic", "vm")]
        );
    });
}

// 435: a node's place is `chain · step n`, `chain › sub · step n` in a subchain, `n.k` for a parallel item; a stack is a called subchain's nodes.
#[test]
fn a_nodes_place_and_the_stacks_of_a_chain_project() {
    let x = "plugin({ type: \"internal\", name: \"X\" })\n";
    let chain = "start(true)\nsteps([id(\"l1\").track(\"a\"), id(\"p\").extends(\"x1\"), [id(\"q\").extends(\"x2\"), id(\"r\").extends(\"x3\")], {sub: \"s\"}, to([chain(\"OUT\")])])\nsubchains([{id: \"s\", steps: [id(\"d\").extends(\"x4\"), id(\"e\").extends(\"x5\")]}])\n";
    let a = lane_a();
    let files = [
        ("chains/main.k7x", chain),
        ("tracks/a.k7x", a.as_str()),
        ("nodes/x.k7x", x),
    ];
    let (src, roots) = graph_support::world(&files);
    let inputs = graph_support::inputs_with(&src, &roots, &serde_json::json!({}));
    let project = inputs.routing.project.as_ref().expect("a chain project");
    let places = song::graph::node_places(project);
    assert_eq!(places["p"].place, "main · step 2");
    assert_eq!(places["p"].uses, Some("x1"));
    assert_eq!(places["q"].place, "main · step 3.1");
    assert_eq!(places["r"].place, "main · step 3.2");
    assert_eq!(places["d"].place, "main › s · step 1");
    assert!(!places.contains_key("a"), "a lane has a name of its own");
    assert_eq!(song::graph::node_stacks(project), [["d".to_owned(), "e".to_owned()]]);
}
