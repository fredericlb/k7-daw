// graph_handed.rs — Part B of 2k: `handed_of` over a compile's answer, the hostile projects it is asked of, and the one call that draws over it.
// A reader would get wrong that the manual plans need a document: they are the graph's own and stand when the compile failed.

use crate::graph_support;

use graph_support::{fast, inputs_with, world};
use serde_json::json;
use song::clips::ClipDoc;
use song::doc::{Compiled, Failure, PluginDoc, ProjectDoc, TrackDoc, frozen_clip};
use song::freeze::{AutoFreeze, FreezePlan};
use song::graph::{Folder, Handed, handed_of, load_compiled_graph, load_graph, web_graph};
use song::lanes::inputs::Inputs;
use song::lanes::rig::Rig;
use song::{Cache, MemSource, Roots};

const MIXER: &str = "Mixer";

fn compiled(doc: Option<ProjectDoc>, freezes: Vec<AutoFreeze>) -> Compiled {
    Compiled {
        engine: doc.ok_or(Failure {
            diagnostics: Vec::new(),
            format_too_new: false,
        }),
        diagnostics: Vec::new(),
        freezes,
        renders_used: Vec::new(),
        spent: song::doc::Spent::default(),
    }
}

fn track(id: &str, plugins: &[(&str, &str)]) -> TrackDoc {
    let mut t = TrackDoc::new(id);
    t.plugins = plugins
        .iter()
        .map(|(kind, name)| {
            let mut p = PluginDoc::new(&format!("{id}:{name}"));
            p.plugin_type = (*kind).to_owned();
            p.name = (*name).to_owned();
            p
        })
        .collect();
    t
}

fn clip(start: f64, length: f64) -> ClipDoc {
    let mut c = frozen_clip("c", "x.wav");
    c.start = start;
    c.length = length;
    c
}

fn doc_of(tracks: Vec<TrackDoc>) -> ProjectDoc {
    let mut d = ProjectDoc::empty();
    d.tracks = tracks;
    d
}

/// Loads the laid files' graph and hands the compile's answer and the source to `f`.
fn handed_over(files: &[(&str, &str)], c: &Compiled, f: impl FnOnce(Handed)) {
    let (src, roots) = world(files);
    let inputs = inputs_with(&src, &roots, &json!({}));
    let g = load_graph(&src, &mut Cache::new(), &roots, &inputs).expect("graph");
    f(handed_of(&src, &inputs, &g, c).expect("handed"));
}

const RUN: &[(&str, &str)] = &[
    ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
    (
        "nodes/amp.k7x",
        "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"renders/amp.wav\")\n",
    ),
    ("nodes/cab.k7x", "plugin({ type: \"internal\", name: \"Chorus\" })\n"),
    (
        "graph.k7x",
        "modules({ gtr: [\"amp\"], amp: [\"cab\"], cab: [\"OUT\"] })\n",
    ),
];

// 430, trackMixers.test.ts "names the mixer of each lane that has one" and "leaves out a lane with no strip".
#[test]
fn the_mixer_of_a_lane_is_named_and_a_lane_without_one_is_left_out() {
    let doc = doc_of(vec![
        track("gt", &[("Reverb", "verb"), (MIXER, "strip")]),
        track("dr", &[("Reverb", "verb")]),
    ]);
    handed_over(RUN, &compiled(Some(doc), Vec::new()), |h| {
        let m = h.mixers.expect("mixers");
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("gt").map(String::as_str), Some("strip"));
    });
}

// 430, trackMixers.test.ts "takes the LAST one, where the level lives" and "answers per lane, and says nothing for a project with none".
#[test]
fn the_last_mixer_is_the_one_and_a_project_with_none_says_nothing() {
    let doc = doc_of(vec![
        track("gt", &[(MIXER, "pre"), ("Reverb", "verb"), (MIXER, "post")]),
        track("dr", &[(MIXER, "b")]),
    ]);
    handed_over(RUN, &compiled(Some(doc), Vec::new()), |h| {
        let m = h.mixers.expect("mixers");
        let got: Vec<(&str, &str)> = m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(got, [("gt", "post"), ("dr", "b")]);
    });
    handed_over(RUN, &compiled(Some(doc_of(Vec::new())), Vec::new()), |h| {
        assert_eq!(h.mixers.map(|m| m.len()), Some(0));
    });
}

// 430, graphDoc.ts `trackColorsOf`: by NAME, the first track with a colour wins, an empty colour is none.
#[test]
fn a_colour_is_the_first_one_a_name_has() {
    let mut a = TrackDoc::new("a");
    a.color = "#111111".into();
    let mut a2 = TrackDoc::new("a2");
    a2.name = "a".into();
    a2.color = "#222222".into();
    let plain = TrackDoc::new("b");
    handed_over(RUN, &compiled(Some(doc_of(vec![plain, a, a2])), Vec::new()), |h| {
        let c = h.colors.expect("colors");
        assert_eq!(c.len(), 1);
        assert_eq!(c.get("a").map(String::as_str), Some("#111111"));
    });
}

// 430, graphDoc.ts `clipExtentsOf`: a track with clips, by id, its clips' start and length.
#[test]
fn extents_are_the_clips_of_each_track_that_has_any() {
    let mut t = TrackDoc::new("a");
    t.clips = vec![clip(0.0, 2.0), clip(4.0, 0.0)];
    let doc = doc_of(vec![t, TrackDoc::new("empty")]);
    handed_over(RUN, &compiled(Some(doc), Vec::new()), |h| {
        let e = h.extents.expect("extents");
        assert_eq!(e.len(), 1);
        let got: Vec<(f64, f64)> = e["a"].iter().map(|x| (x.start, x.length)).collect();
        assert_eq!(got, [(0.0, 2.0), (4.0, 0.0)]);
    });
}

// 412, jsorder: an integer-like track id is met first, as `Object.entries` lists a record the TypeScript made.
#[test]
fn integer_like_ids_come_first_as_javascript_lists_them() {
    let (mut a, mut n, mut m) = (
        track("a", &[(MIXER, "sa")]),
        track("10", &[(MIXER, "s10")]),
        track("9", &[(MIXER, "s9")]),
    );
    for t in [&mut a, &mut n, &mut m] {
        t.clips = vec![clip(0.0, 1.0)];
    }
    handed_over(RUN, &compiled(Some(doc_of(vec![a, n, m])), Vec::new()), |h| {
        let keys = |m: Vec<&String>| m.into_iter().cloned().collect::<Vec<_>>();
        assert_eq!(keys(h.mixers.as_ref().expect("m").keys().collect()), ["9", "10", "a"]);
        assert_eq!(keys(h.extents.as_ref().expect("e").keys().collect()), ["9", "10", "a"]);
    });
}

fn freeze(module: &str, ready: bool, hosted: bool) -> AutoFreeze {
    AutoFreeze {
        module: module.into(),
        render: "/r.wav".into(),
        plan: FreezePlan {
            at: module.into(),
            render: "/r.wav".into(),
            lanes: ["a".to_owned()].into_iter().collect(),
            modules: [module.to_owned(), "m".to_owned()].into_iter().collect(),
        },
        cut: ProjectDoc::empty(),
        plugins: Vec::new(),
        ready,
        hosted,
    }
}

// 431, freeze.test.ts: a project that did not compile has no extents, mixers, colours or automatic freezes, and still its manual plans.
#[test]
fn a_project_that_does_not_compile_hands_nothing_of_the_compile_but_its_own_plans() {
    let mut files = RUN.to_vec();
    files.push(("renders/amp.wav", "RIFF"));
    handed_over(&files, &compiled(None, vec![freeze("v", true, false)]), |h| {
        assert!(h.extents.is_none() && h.mixers.is_none() && h.colors.is_none());
        assert!(h.auto.is_empty());
        let plans: Vec<(&str, &[String], &[String])> = h
            .manual
            .iter()
            .map(|p| (p.at.as_str(), &p.lanes[..], &p.modules[..]))
            .collect();
        assert_eq!(plans, [("amp", &["gtr".to_owned()][..], &["amp".to_owned()][..])]);
    });
}

// 431: an automatic freeze is copied whole, `ready` and `hosted` as the compile said them, lanes then modules in order.
#[test]
fn an_automatic_freeze_is_copied_as_the_compile_made_it() {
    let fs = vec![freeze("v1", true, false), freeze("v2", false, true)];
    handed_over(RUN, &compiled(Some(doc_of(Vec::new())), fs), |h| {
        assert_eq!(h.auto.len(), 2);
        let a = &h.auto[0];
        assert_eq!((a.module.as_str(), a.ready, a.hosted), ("v1", true, false));
        assert_eq!(
            (&a.lanes[..], &a.modules[..]),
            (&["a".to_owned()][..], &["v1".to_owned(), "m".to_owned()][..])
        );
        assert_eq!((h.auto[1].ready, h.auto[1].hosted), (false, true));
    });
}

// 430, freeze.test.ts "activeFreezePlans": a render there plans the freeze, a render missing plays live (an empty `frozen` is a schema error).
#[test]
fn a_plan_is_in_force_only_over_a_render_that_is_there() {
    let there = {
        let mut f = RUN.to_vec();
        f.push(("renders/amp.wav", "RIFF"));
        f
    };
    handed_over(&there, &compiled(None, Vec::new()), |h| {
        assert_eq!(h.manual.iter().map(|p| p.at.as_str()).collect::<Vec<_>>(), ["amp"]);
    });
    handed_over(RUN, &compiled(None, Vec::new()), |h| assert!(h.manual.is_empty()));
}

// 431: a plan splits what is inside by the GRAPH's lanes, lanes first, and a later module's plan comes in the modules' order.
#[test]
fn a_plan_over_the_graphs_chains_names_its_lanes_and_modules() {
    let files = [
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        (
            "nodes/amp.k7x",
            "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"r.wav\")\n",
        ),
        (
            "nodes/cab.k7x",
            "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"r.wav\")\n",
        ),
        ("r.wav", "RIFF"),
        (
            "graph.k7x",
            "modules({ gtr: [\"amp\"], amp: [\"cab\"], cab: [\"OUT\"] })\n",
        ),
    ];
    handed_over(&files, &compiled(None, Vec::new()), |h| {
        let got: Vec<(&str, Vec<&str>)> = h
            .manual
            .iter()
            .map(|p| {
                (
                    p.at.as_str(),
                    p.lanes.iter().chain(&p.modules).map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "amp");
        assert_eq!(got[1].0, "cab");
        assert_eq!(got[1].1.len(), 3);
        assert_eq!(got[1].1[0], "gtr");
    });
}

// 462: the one call a host makes runs the four steps, so the plans stand beside a compile that failed and the open project's freezes are its own.
#[test]
fn the_compiled_graph_is_drawn_over_what_the_compile_hands_and_over_its_own_plans() {
    let mut files = RUN.to_vec();
    files.push(("renders/amp.wav", "RIFF"));
    let (src, roots) = world(&files);
    let c = compiled(Some(lane_doc("gtr")), vec![freeze("v", true, false)]);
    let draw = |c: Option<&Compiled>, folder| {
        let a = load_compiled_graph(&src, &mut Cache::new(), &roots, Rig::default(), c, folder).expect("answer");
        assert!(!a.capped);
        serde_json::to_value(&a.graph).expect("json")
    };
    let open = draw(Some(&c), Folder::Open);
    assert_eq!(open["frozen"]["amp"], "at");
    assert_eq!(open["autoFrozen"]["v"], "v");
    assert_eq!(open["trackMixers"]["gtr"], "strip");
    assert_eq!(open["trackColors"]["gtr"], "#123456");
    // The same four steps by hand give the same graph.
    let inputs = inputs_with(&src, &roots, &json!({}));
    let g = load_graph(&src, &mut Cache::new(), &roots, &inputs).expect("graph");
    let handed = handed_of(&src, &inputs, &g, &c).expect("handed");
    let by_hand = web_graph(&src, &mut Cache::new(), &roots, &inputs, &g, &handed).expect("drawn");
    assert_eq!(serde_json::to_value(&by_hand.graph).expect("json"), open);
    // Another folder draws no automatic freeze, but still its own plans and the compile's values.
    let other = draw(Some(&c), Folder::Other);
    assert!(other.get("autoFrozen").is_none() && other.get("autoFreezeWaiting").is_none());
    assert_eq!(
        (&other["frozen"]["amp"], &other["trackMixers"]["gtr"]),
        (&json!("at"), &json!("strip"))
    );
    // A project that did not compile draws its plans and none of the compile's values.
    for none in [
        draw(None, Folder::Open),
        draw(Some(&compiled(None, vec![freeze("v", true, false)])), Folder::Open),
    ] {
        assert_eq!(none["frozen"]["amp"], "at");
        assert!(none.get("trackMixers").is_none() && none.get("autoFrozen").is_none());
        assert_eq!(none["trackColors"].as_object().map(|c| c.len()), Some(0));
    }
}

const N: usize = 100_000;

/// A project of one lane whose run is `chain` modules named `<pad>f<n>`, each frozen at a render that exists.
fn frozen_run(chain: usize, lane_name: &str, pad: &str) -> (MemSource, Roots, Inputs) {
    let (src, roots) = world(&[
        ("nodes/lane.k7x", "extends(\"@meta/track\")\n"),
        (
            "nodes/f.k7x",
            "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"r.wav\")\n",
        ),
        ("r.wav", "RIFF"),
        ("graph.k7x", "modules({ lane: [\"OUT\"] })\n"),
    ]);
    let mut i = inputs_with(&src, &roots, &json!({}));
    let proto = i.local.modules.get("f").expect("f").clone();
    let lane = i.local.modules.get("lane").expect("lane").clone();
    i.local.modules.shift_remove("lane");
    i.local.modules.insert(lane_name.to_owned(), lane);
    for n in 0..chain {
        i.local.modules.insert(format!("{pad}f{n}"), proto.clone());
    }
    (src, roots, i)
}

fn run_of(lane: &str, chain: usize, pad: &str) -> song::routing::Chain {
    song::routing::Chain {
        from: Some(lane.to_owned()),
        from_port: None,
        use_steps: (0..chain)
            .map(|n| song::routing::UseStep {
                module: format!("{pad}f{n}"),
                inputs: None,
                enabled: None,
            })
            .collect(),
        out: vec![song::routing::Out {
            to: "OUT".to_owned(),
            gain: 1.0,
            from_port: None,
        }],
    }
}

/// The `Handed` of the run `frozen_run` makes, drawn over the compile `c`.
fn handed_of_run(chain: usize, lane: &str, pad: &str, c: &Compiled) -> Handed {
    let (src, roots, i) = frozen_run(chain, lane, pad);
    let mut g = load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph");
    g.chains.insert(lane.into(), run_of(lane, chain, pad));
    fast("handed", 5, || handed_of(&src, &i, &g, c).expect("handed"))
}

// 419 over the Handed's own copies: 100,000 frozen modules in one run ask for plans of 50,000 names each; the freeze budget stops them, whole.
#[test]
fn a_hundred_thousand_frozen_modules_are_stopped_by_the_budget() {
    // The same shape at three modules gives three plans: the empty answer below is the budget's, not the setup's.
    let c = compiled(None, Vec::new());
    assert_eq!(handed_of_run(3, "lane", "", &c).manual.len(), 3);
    let h = handed_of_run(N, "lane", "", &c);
    assert!(h.manual.is_empty(), "{} plans", h.manual.len());
}

// 459: a lane named by a megabyte is copied into every plan and each copy is paid by its bytes, so 300 plans are none, not a few dozen.
#[test]
fn a_name_of_a_megabyte_in_every_plan_stops_the_plans_whole() {
    let big = "n".repeat(1 << 20);
    let c = compiled(None, Vec::new());
    let few = handed_of_run(20, &big, "", &c);
    assert_eq!(few.manual.len(), 20);
    assert!(few.manual.iter().all(|p| p.lanes.first() == Some(&big)));
    assert!(handed_of_run(300, &big, "", &c).manual.is_empty());
}

/// A document with one lane that has a clip, a mixer and a colour.
fn lane_doc(id: &str) -> ProjectDoc {
    let mut t = track(id, &[(MIXER, "strip")]);
    t.name = id.to_owned();
    t.clips = vec![clip(0.0, 2.0)];
    t.color = "#123456".into();
    doc_of(vec![t])
}

// 463: 3,000 frozen modules named by 200 bytes give no plans at all, never a cut list; the compile's values, paid first, stay whole.
#[test]
fn many_long_names_give_no_plans_and_the_compiles_values_whole() {
    let c = compiled(Some(lane_doc("gtr")), Vec::new());
    let h = handed_of_run(3_000, "lane", &"p".repeat(200), &c);
    assert!(h.manual.is_empty(), "{} plans", h.manual.len());
    assert_eq!(h.extents.map(|e| e.len()), Some(1));
    assert_eq!(h.mixers.map(|m| m.len()), Some(1));
    assert_eq!(h.colors.map(|m| m.len()), Some(1));
}

/// A project of one lane `lane` and a run of `n` frozen modules named `<pad>f<n>`, as files.
fn frozen_files(n: usize, pad: &str) -> Vec<(String, String)> {
    let mut files = vec![
        ("r.wav".to_owned(), "RIFF".to_owned()),
        ("nodes/lane.k7x".to_owned(), "extends(\"@meta/track\")\n".to_owned()),
    ];
    let mut pairs = vec![format!("\"lane\": [\"{pad}f0\"]")];
    for i in 0..n {
        let to = if i + 1 < n {
            format!("{pad}f{}", i + 1)
        } else {
            "OUT".to_owned()
        };
        pairs.push(format!("\"{pad}f{i}\": [\"{to}\"]"));
        files.push((
            format!("nodes/{pad}f{i}.k7x"),
            "plugin({ type: \"internal\", name: \"Chorus\" })\nfrozen(\"r.wav\")\n".to_owned(),
        ));
    }
    files.push(("graph.k7x".to_owned(), format!("modules({{ {} }})\n", pairs.join(", "))));
    files
}

// 463: a stop of the answer's own spend leaves the plans out whole, keeps the compile's values and says so, capped.
#[test]
fn a_stop_of_the_spend_is_said_by_the_answer_and_costs_the_plans_only() {
    let pad = "p".repeat(200);
    let c = compiled(Some(lane_doc("lane")), Vec::new());
    let ask = |n: usize| {
        let owned = frozen_files(n, &pad);
        let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let (src, roots) = world(&files);
        fast("answer", 5, || {
            load_compiled_graph(&src, &mut Cache::new(), &roots, Rig::default(), Some(&c), Folder::Open)
                .expect("answer")
        })
    };
    // 900 plans of up to 900 names of 200 bytes are 1.6 million units, over the spend and under the freeze budget.
    let a = ask(900);
    assert!(a.capped && a.findings.iter().any(|f| f.file == "project.k7x"));
    assert!(a.graph.frozen.is_none());
    assert_eq!(
        a.graph
            .track_mixers
            .as_ref()
            .and_then(|m| m.get("lane"))
            .map(String::as_str),
        Some("strip")
    );
    assert_eq!(a.graph.track_colors.get("lane").map(String::as_str), Some("#123456"));
    // The same project with 100 modules draws its plans and is not capped.
    let a = ask(100);
    assert!(!a.capped && a.graph.frozen.is_some());
}

// 419: a document of 100,000 clips on one track and of 100,000 tracks is copied once, in time, and every extent kept.
#[test]
fn a_big_document_is_copied_once_and_whole() {
    let mut t = TrackDoc::new("a");
    t.clips = (0..N).map(|n| clip(n as f64, 1.0)).collect();
    let many: Vec<TrackDoc> = (0..N).map(|n| track(&format!("t{n}"), &[(MIXER, "m")])).collect();
    let (src, roots) = world(RUN);
    let i = inputs_with(&src, &roots, &json!({}));
    let g = load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph");
    let h = fast("handed", 5, || {
        handed_of(&src, &i, &g, &compiled(Some(doc_of(vec![t])), Vec::new())).expect("handed")
    });
    assert_eq!(h.extents.expect("extents")["a"].len(), N);
    let h = fast("handed", 5, || {
        handed_of(&src, &i, &g, &compiled(Some(doc_of(many)), Vec::new())).expect("handed")
    });
    assert_eq!(h.mixers.expect("mixers").len(), N);
}
