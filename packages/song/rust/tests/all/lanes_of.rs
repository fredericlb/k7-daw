// lanes_of.rs — the lane projection (decisions 241 to 244) and what a lane carries: trackModule.test.ts, clipWindow.test.ts.
// A reader would get wrong that nothing is copied: a lane's events are the resolved module's own, by pointer.

use crate::lanes_support;

use lanes_support::{fast, inputs, lane};
use song::Placement;
use song::lanes::{LaneKind, Material, is_pattern_lane, lane_materials, lane_signal, pattern_lanes};

fn kinds(files: &[(&str, &str)]) -> Vec<(String, LaneKind)> {
    let i = inputs(files);
    i.lanes().sound.iter().map(|(n, l)| ((*n).to_owned(), l.kind)).collect()
}

// 241: precedence group, strudel, picture, else single; a node extending each kind.
#[test]
fn each_of_the_four_kinds_is_told_by_what_the_node_extends() {
    let i = inputs(&[
        ("nodes/a.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/b.k7x", "extends(\"@meta/group-track\")\ntracks([\"a\"])\n"),
        (
            "nodes/c.k7x",
            "extends(\"@meta/strudel-track\")\nstrudel(\"algo/x.strudel\")\n",
        ),
        ("nodes/d.k7x", "extends(\"@meta/picture-track\")\n"),
    ]);
    let lanes = i.lanes();
    let sound: Vec<_> = lanes.sound.iter().map(|(n, l)| (*n, l.kind)).collect();
    assert_eq!(
        sound,
        [
            ("a", LaneKind::Single),
            ("b", LaneKind::Group),
            ("c", LaneKind::Strudel)
        ]
    );
    assert_eq!(lanes.picture.keys().copied().collect::<Vec<_>>(), ["d"]);
    assert_eq!(lanes.sound["b"].tracks, ["a"]);
}

// 241: color empty is absent; events empty for a group and a pattern lane; video_source only on single and picture.
#[test]
fn the_fields_a_kind_does_not_have_are_empty() {
    let ev = "events([ref(\"k\").start(\"1:1\")])\n";
    let i = inputs(&[
        (
            "nodes/s.k7x",
            &lane(&format!("color(\"#ff0000\")\nvideo_source(\"cam\")\n{ev}")),
        ),
        (
            "nodes/g.k7x",
            &format!("extends(\"@meta/group-track\")\n{ev}tracks([\"s\"])\nvideo_source(\"cam\")\n"),
        ),
        (
            "nodes/p.k7x",
            &format!("extends(\"@meta/strudel-track\")\nstrudel(\"algo/x.strudel\")\n{ev}video_source(\"cam\")\n"),
        ),
        ("nodes/e.k7x", &lane("color(\"\")\n")),
    ]);
    let l = i.lanes();
    let s = &l.sound["s"];
    assert_eq!(
        (s.color, s.video_source, s.events.len()),
        (Some("#ff0000"), Some("cam"), 1)
    );
    for name in ["g", "p"] {
        assert!(l.sound[name].events.is_empty(), "{name}");
        assert_eq!(l.sound[name].video_source, None, "{name}");
    }
    assert_eq!(
        (l.sound["g"].strudel, l.sound["p"].strudel),
        (None, Some("algo/x.strudel"))
    );
    assert_eq!(l.sound["e"].color, None);
    // The serialized lane carries the keys laneOf writes, `events` and `tracks` always.
    let v = serde_json::to_value(&l.sound["e"]).expect("json");
    assert_eq!(
        v,
        serde_json::json!({"type": "single", "events": [], "tracks": [], "disabled": false})
    );
}

// 241: `frame` rides a picture lane only; a sound lane's frame is dropped; `disabled` is read.
#[test]
fn a_frame_rides_a_picture_lane_alone_and_disabled_is_read() {
    let i = inputs(&[
        (
            "nodes/pic.k7x",
            "extends(\"@meta/picture-track\")\nframe([0, 0, 640, 360])\n",
        ),
        ("nodes/snd.k7x", &lane("frame([0, 0, 1, 1])\ndisabled(true)\n")),
    ]);
    let l = i.lanes();
    assert_eq!(l.picture["pic"].frame, Some([0.0, 0.0, 640.0, 360.0]));
    assert_eq!((l.sound["snd"].frame, l.sound["snd"].disabled), (None, true));
}

// 242: a pattern lane is a strudel lane with a path that is not disabled; the list is in the map's order.
#[test]
fn a_strudel_lane_without_a_pattern_or_disabled_is_no_pattern_lane() {
    let i = inputs(&[
        (
            "nodes/b.k7x",
            "extends(\"@meta/strudel-track\")\nstrudel(\"algo/b.strudel\")\n",
        ),
        ("nodes/a.k7x", "extends(\"@meta/strudel-track\")\n"),
        (
            "nodes/c.k7x",
            "extends(\"@meta/strudel-track\")\nstrudel(\"algo/c.strudel\")\ndisabled(true)\n",
        ),
        (
            "nodes/d.k7x",
            "extends(\"@meta/strudel-track\")\nstrudel(\"algo/d.strudel\")\n",
        ),
    ]);
    let l = i.lanes();
    assert!(!is_pattern_lane(&l.sound["a"]) && !is_pattern_lane(&l.sound["c"]));
    assert_eq!(pattern_lanes(&l), [("b", "algo/b.strudel"), ("d", "algo/d.strudel")]);
}

// 244: a lane is a project node; a chain step naming `@meta/track` is not one.
#[test]
fn a_chain_step_naming_a_meta_track_is_not_a_lane() {
    let i = inputs(&[
        ("nodes/real.k7x", &lane("")),
        (
            "graph.k7x",
            "modules({ real: [\"@meta/track\"], \"@meta/track\": [\"OUT\"] })\n",
        ),
    ]);
    assert_eq!(i.lanes().sound.keys().copied().collect::<Vec<_>>(), ["real"]);
    assert!(i.extra.modules.contains_key("@meta/track"));
    assert!(!i.lanes().sound.contains_key("@meta/track"));
    assert_eq!(kinds(&[("nodes/real.k7x", &lane(""))]).len(), 1);
}

// 241: the projection copies nothing: `events` is the resolved module's own slice.
#[test]
fn the_events_of_a_lane_are_the_resolved_modules_own() {
    let i = inputs(&[(
        "nodes/a.k7x",
        &lane("events([ref(\"k\").start(\"1:1\"), ref(\"k\").start(\"2:1\")])\n"),
    )]);
    let l = i.lanes();
    let own = i.local.modules["a"].authored.events.as_deref().expect("events");
    assert!(std::ptr::eq(l.sound["a"].events.as_ptr(), own.as_ptr()));
    let big: String = (0..100_000).map(|n| format!("ref(\"k\").start({n}),")).collect();
    let i = inputs(&[("nodes/big.k7x", &lane(&format!("events([{big}])\n")))]);
    let n = fast("100,000 events", 5, || i.lanes().sound["big"].events.len());
    assert_eq!(n, 100_000);
}

fn ev(r: Option<&str>) -> Placement {
    serde_json::from_value(r.map_or_else(
        || serde_json::json!({"start": "1:1"}),
        |r| serde_json::json!({"ref": r}),
    ))
    .expect("a placement")
}

// 243: the materials of a lane and the signal it puts on a wire.
#[test]
fn materials_and_the_signal_of_a_lane() {
    let material = |r: &str| match r {
        "wav" => Material::Audio,
        "mid" => Material::Midi,
        _ => Material::Missing,
    };
    let sig = |refs: &[Option<&str>]| {
        let events: Vec<Placement> = refs.iter().map(|r| ev(*r)).collect();
        lane_signal(&lane_materials(&events, &material))
    };
    assert_eq!(sig(&[Some("wav"), Some("wav")]), Some(Material::Audio));
    assert_eq!(sig(&[Some("mid")]), Some(Material::Midi));
    assert_eq!(sig(&[Some("wav"), Some("mid")]), None);
    assert_eq!(sig(&[Some("wav"), Some("nope")]), None);
    assert_eq!(sig(&[]), None);
    // An absent `ref` is asked as "" and is missing.
    assert_eq!(sig(&[None]), None);
    assert!(lane_materials(&[ev(None)], &material).missing);
}

/// A project of one lane copied `n` times; the copies are the resolved map's own, so nothing is shared between them.
fn many_lanes(n: usize, events: usize) -> song::lanes::inputs::Inputs {
    let list: String = (0..events).map(|k| format!("ref(\"k\").start({k}),")).collect();
    let mut i = inputs(&[("nodes/l0.k7x", &lane(&format!("events([{list}])\n")))]);
    let one = i.local.modules["l0"].clone();
    for k in 1..n {
        i.local.modules.insert(format!("l{k}"), one.clone());
    }
    i
}

// 283: the brief's size, 100,000 lanes of 100 events: the projection copies none of the 10 million (measured, run by hand).
#[test]
#[ignore = "10 million events, several GB; run by hand"]
fn a_hundred_thousand_lanes_of_a_hundred_events() {
    eprintln!("size_of Placement = {}", std::mem::size_of::<Placement>());
    let i = many_lanes(100_000, 100);
    let lanes = fast("100,000 lanes", 20, || i.lanes());
    assert_eq!(lanes.sound.len(), 100_000);
    assert!(lanes.sound.values().all(|l| l.events.len() == 100));
    let own = i.local.modules["l99999"].authored.events.as_deref().expect("events");
    assert!(std::ptr::eq(lanes.sound["l99999"].events.as_ptr(), own.as_ptr()));
}

// 283: 100,000 lanes of 5 events in every run: the projection is a borrow, so it costs one pass.
#[test]
fn a_hundred_thousand_lanes_are_projected_in_one_pass() {
    let i = many_lanes(100_000, 5);
    let lanes = fast("100,000 lanes", 5, || i.lanes());
    assert_eq!(lanes.sound.len(), 100_000);
    assert!(lanes.sound.values().all(|l| l.events.len() == 5));
    let own = i.local.modules["l99999"].authored.events.as_deref().expect("events");
    assert!(std::ptr::eq(lanes.sound["l99999"].events.as_ptr(), own.as_ptr()));
}

// 283: 10,000 nodes extending one base of events, so the resolver copies the events into each up to its budget; the projection copies none.
#[test]
fn lanes_filled_by_the_resolver_up_to_its_budget_are_borrowed() {
    let events: String = (0..90).map(|k| format!("ref(\"k\").start({k}),")).collect();
    let base = format!("events([{events}])\n");
    let nodes: Vec<(String, String)> = (0..10_000)
        .map(|k| {
            (
                format!("nodes/n{k}.k7x"),
                "extends(\"@k7-core/tracks/base\")\n".to_owned(),
            )
        })
        .collect();
    let mut files: Vec<(&str, &str)> = vec![("lib/k7-core/modules/tracks/base/module.k7x", &base)];
    files.extend(nodes.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    let i = inputs(&files);
    let lanes = fast("10,000 filled lanes", 5, || i.lanes());
    assert!(
        !i.local.capped && i.local.weight > 900_000,
        "the resolver spent {}",
        i.local.weight
    );
    assert_eq!(lanes.sound.len(), 10_000);
    let own = i.local.modules["n9999"].authored.events.as_deref().expect("events");
    assert!(std::ptr::eq(lanes.sound["n9999"].events.as_ptr(), own.as_ptr()));
}
