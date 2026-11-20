// graph_lanes.rs — what each lane shows in the WebGraph: clips with their widths, signals, colours, strips, freezes, a live input's tap, a chain project's places.
// A reader would get wrong that a clip's width comes from the COMPILE (`Handed`): a project that does not compile draws clips without one (430).

use crate::graph_support;

use graph_support::{Drawn, OUT_AMP, draw, draw_with, lane, no_null};
use serde_json::{Value, json};
use song::graph::{AutoMark, ClipExtent, FreezeMark, Handed};

fn at<'a>(d: &'a Drawn, ptr: &str) -> Option<&'a Value> {
    d.json.pointer(ptr)
}

fn clean(d: &Drawn) {
    let mut bad = Vec::new();
    no_null(&d.json, "", &mut bad);
    assert!(bad.is_empty(), "{bad:?}");
}

// trackClips.test.ts: sorted by position, spelled by the meter, missing says so, absent for an empty lane, widths from the compile.
#[test]
fn a_lanes_clips_are_sorted_spelled_by_the_meter_and_sized_by_the_compile() {
    let l = lane(
        "events([ref(\"b\").start(\"3:1\"), ref(\"a\").start(\"1:1\"), ref(\"nope\").start(\"2:1\"), ref(\"z\").start(\"4:1\")])\n",
    );
    let files = [
        ("project.k7x", "project({ name: \"t\", timeSignature: \"7/8\" })\n"),
        ("clips/a.k7x", "source(\"clips/a.wav\")\n"),
        ("sequences/b.k7x", "notes([t(0).d(1).k(60)])\n"),
        ("clips/z.k7x", "source(\"clips/z.wav\")\n"),
        ("nodes/m.k7x", l.as_str()),
        ("nodes/quiet.k7x", "extends(\"@meta/track\")\n"),
        ("graph.k7x", "modules({ m: [\"OUT\"], quiet: [\"OUT\"] })\n"),
    ];
    let d = draw(&files);
    let clips = at(&d, "/trackClips/m").and_then(Value::as_array).expect("clips");
    let order: Vec<&str> = clips.iter().map(|c| c["ref"].as_str().expect("ref")).collect();
    assert_eq!(order, ["a", "nope", "b", "z"]);
    assert_eq!(clips[0]["tc"], json!("1:1"));
    assert_eq!(clips[1]["kind"], json!("missing"));
    // 7/8 has a bar of 3.5 quarter beats, so bar 2 begins at 3.5.
    assert!((clips[1]["start"].as_f64().expect("start") - 3.5).abs() < 1e-9);
    assert_eq!(clips[1]["tc"], json!("2:1"));
    assert!(at(&d, "/trackClips/quiet").is_none());
    assert!(clips.iter().all(|c| c.get("len").is_none()), "no extents were handed");
    let handed = Handed {
        extents: Some(
            [(
                "m".to_owned(),
                vec![
                    ClipExtent {
                        start: 0.0,
                        length: 2.0,
                    },
                    ClipExtent {
                        start: 3.5,
                        length: 0.0,
                    },
                    ClipExtent {
                        start: 99.0,
                        length: 9.0,
                    },
                ],
            )]
            .into_iter()
            .collect(),
        ),
        ..Handed::default()
    };
    let d = draw_with(&files, &json!({}), &handed);
    let clips = at(&d, "/trackClips/m").and_then(Value::as_array).expect("clips");
    assert_eq!(clips[0]["len"], json!(2.0));
    assert_eq!(clips[1]["len"], json!(4.0), "a length of 0 draws as four beats");
    assert!(
        clips[2].get("len").is_none(),
        "a placement the compile does not know has no length"
    );
    clean(&d);
}

// 432 and 430: the extent keyed by the start's bits, the last of two at one start winning; a NaN length drops the len.
#[test]
fn an_extent_is_matched_by_start_the_last_winning_and_a_nan_length_drops() {
    let l = lane("events([ref(\"a\").start(\"1:1\"), ref(\"a\").start(\"2:1\")])\n");
    let files = [
        ("clips/a.k7x", "source(\"clips/a.wav\")\n"),
        ("nodes/m.k7x", l.as_str()),
        ("graph.k7x", "modules({ m: [\"OUT\"] })\n"),
    ];
    let handed = Handed {
        extents: Some(
            [(
                "m".to_owned(),
                vec![
                    ClipExtent {
                        start: -0.0,
                        length: 1.0,
                    },
                    ClipExtent {
                        start: 0.0,
                        length: 3.0,
                    },
                    ClipExtent {
                        start: 4.0,
                        length: f64::NAN,
                    },
                ],
            )]
            .into_iter()
            .collect(),
        ),
        ..Handed::default()
    };
    let d = draw_with(&files, &json!({}), &handed);
    let clips = at(&d, "/trackClips/m").and_then(Value::as_array).expect("clips");
    assert_eq!(clips[0]["len"], json!(3.0));
    assert!(clips[1].get("len").is_none());
}

// inputWidgets.test.ts: a monitor, the tap on the first plugin of the lane's run, none when the lane runs none.
#[test]
fn a_live_input_carries_its_monitor_and_listens_to_the_first_plugin_of_its_lane() {
    let rec = "io(\"in\")\nchannel([1])\nmonitor(true)\n";
    let l = lane("");
    let d = draw(&[
        ("nodes/mic.k7x", l.as_str()),
        ("nodes/amp.k7x", OUT_AMP),
        ("nodes/rec.k7x", rec),
        (
            "graph.k7x",
            "modules({ mic: [\"amp\"], amp: [\"OUT\"], rec: [\"mic\"] })\n",
        ),
    ]);
    assert_eq!(at(&d, "/chains/rec/use/0/monitor"), Some(&json!("on")));
    assert_eq!(at(&d, "/chains/rec/use/0/channel"), Some(&json!([1])));
    assert_eq!(
        at(&d, "/chains/rec/use/0/tap"),
        Some(&json!({"id": "mic:amp", "side": "input"}))
    );
    assert_eq!(at(&d, "/chains/mic/use/0/monitor"), None);
    // A lane that runs no plugin has nothing to listen to.
    let d = draw(&[
        ("nodes/mic.k7x", l.as_str()),
        ("nodes/rec.k7x", rec),
        ("graph.k7x", "modules({ mic: [\"OUT\"], rec: [\"mic\"] })\n"),
    ]);
    assert_eq!(at(&d, "/chains/rec/use/0/tap"), None);
    // An input that sits INSIDE the lane's run listens too.
    let d = draw(&[
        ("nodes/mic.k7x", l.as_str()),
        ("nodes/amp.k7x", OUT_AMP),
        ("nodes/rec.k7x", rec),
        (
            "graph.k7x",
            "modules({ mic: [\"rec\"], rec: [\"amp\"], amp: [\"OUT\"] })\n",
        ),
    ]);
    assert_eq!(
        at(&d, "/chains/mic/use/0/tap"),
        Some(&json!({"id": "mic:amp", "side": "input"}))
    );
}

// The lane's face: colours (its own, else the compile's), signals, strips pinned from the graph for a picture lane, the compile's for a sound one.
#[test]
fn lanes_have_colours_signals_alternatives_and_strips() {
    let a = lane("color(\"#ff0000\")\n");
    let m = lane("events([ref(\"r\").start(\"1:1\")])\n");
    let handed = Handed {
        colors: Some(
            [
                ("m".to_owned(), "#00ff00".to_owned()),
                ("a".to_owned(), "#111111".to_owned()),
            ]
            .into_iter()
            .collect(),
        ),
        mixers: Some(
            [
                ("a".to_owned(), "mix-a".to_owned()),
                ("ghost".to_owned(), "mix-g".to_owned()),
            ]
            .into_iter()
            .collect(),
        ),
        ..Handed::default()
    };
    let d = draw_with(
        &[
            ("meta/shaders/video-mixer/module.k7x", "type(\"shader\")\n"),
            ("sequences/r.k7x", "notes([t(0).d(1).k(60)])\n"),
            ("nodes/a.k7x", a.as_str()),
            ("nodes/m.k7x", m.as_str()),
            ("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n"),
            ("nodes/vm.k7x", "extends(\"@meta/shaders/video-mixer\")\n"),
            (
                "graph.k7x",
                "modules({ a: [\"OUT\"], m: [\"OUT\"], pic: [\"vm\"], vm: [\"RENDER\"] })\n",
            ),
        ],
        &json!({}),
        &handed,
    );
    assert_eq!(at(&d, "/trackColors"), Some(&json!({"a": "#ff0000", "m": "#00ff00"})));
    assert_eq!(at(&d, "/trackSignals"), Some(&json!({"a": "audio", "m": "midi"})));
    assert_eq!(at(&d, "/tracks"), Some(&json!(["a", "m"])));
    assert_eq!(at(&d, "/videoTracks"), Some(&json!(["pic"])));
    assert_eq!(at(&d, "/trackMixers"), Some(&json!({"a": "mix-a", "pic": "vm"})));
    clean(&d);
}

// 431: a plan's lanes and modules inside, then its point; a later plan may overwrite; the automatic marks by ready and hosted.
#[test]
fn frozen_marks_come_from_the_handed_plans_and_the_automatic_freezes() {
    let handed = Handed {
        manual: vec![
            FreezeMark {
                at: "x".into(),
                lanes: vec!["l".into()],
                modules: vec!["m".into()],
            },
            FreezeMark {
                at: "m".into(),
                lanes: vec![],
                modules: vec!["n".into()],
            },
        ],
        auto: vec![
            AutoMark {
                module: "vst1".into(),
                ready: true,
                hosted: false,
                lanes: vec!["a".into()],
                modules: vec![],
            },
            AutoMark {
                module: "vst2".into(),
                ready: false,
                hosted: false,
                lanes: vec![],
                modules: vec!["b".into()],
            },
            AutoMark {
                module: "vst3".into(),
                ready: false,
                hosted: true,
                lanes: vec!["c".into()],
                modules: vec![],
            },
        ],
        ..Handed::default()
    };
    let d = draw_with(&[("graph.k7x", "modules({ a: [\"OUT\"] })\n")], &json!({}), &handed);
    assert_eq!(
        at(&d, "/frozen"),
        Some(&json!({"l": "inside", "m": "at", "n": "inside", "x": "at"}))
    );
    assert_eq!(at(&d, "/autoFrozen"), Some(&json!({"a": "vst1"})));
    assert_eq!(at(&d, "/autoFreezeWaiting"), Some(&json!({"b": "vst2"})));
    let a = lane("");
    let d = draw(&[
        ("nodes/a.k7x", a.as_str()),
        ("graph.k7x", "modules({ a: [\"OUT\"] })\n"),
    ]);
    for k in [
        "/frozen",
        "/autoFrozen",
        "/autoFreezeWaiting",
        "/trackMixers",
        "/trackClips",
        "/curves",
        "/assemblies",
        "/issues",
        "/tapFeeds",
        "/positions",
        "/stacks",
    ] {
        assert_eq!(at(&d, k), None, "{k} is absent when empty");
    }
    assert_eq!(at(&d, "/modules"), Some(&json!({})));
}

// linkLevels.test.ts "is drawn on the graph's out" and fromPort on a cable; stacks and places in a chain project.
#[test]
fn a_chain_project_draws_levels_places_and_stacks() {
    let l = lane("");
    let chain = "start(true)\nsteps([id(\"l1\").track(\"a\"), id(\"p\").plugin({ type: \"internal\", name: \"Chorus\" }), {sub: \"s\"}, to([chain(\"OUT\").level(0.5)])])\nsubchains([{id: \"s\", steps: [id(\"d\").plugin({ type: \"internal\", name: \"Chorus\" })]}])\n";
    let d = draw(&[("chains/main.k7x", chain), ("tracks/a.k7x", l.as_str())]);
    let gains: Vec<f64> = d
        .json
        .pointer("/chains")
        .and_then(Value::as_object)
        .expect("chains")
        .values()
        .flat_map(|c| {
            c["out"]
                .as_array()
                .expect("out")
                .iter()
                .map(|o| o["gain"].as_f64().expect("gain"))
        })
        .collect();
    assert!(gains.contains(&0.5), "{gains:?}");
    assert_eq!(at(&d, "/stacks"), Some(&json!([["d"]])));
    let placed = d.json.pointer("/chains").and_then(Value::as_object).expect("chains");
    let places: Vec<&str> = placed
        .values()
        .flat_map(|c| {
            c["use"]
                .as_array()
                .expect("use")
                .iter()
                .filter_map(|s| s["place"].as_str())
        })
        .collect();
    assert!(places.contains(&"main · step 2"), "{places:?}");
    assert!(places.contains(&"main › s · step 1"), "{places:?}");
    clean(&d);
}
