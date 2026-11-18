// graph_capacity.rs — a real-size project passes the budget uncapped, with the spend printed and bounded; hostile output is capped (419, 420).
// A reader would get wrong that a limit is hostile only by being small: these projects are the sizes a user really has.

use crate::graph_support;

use graph_support::{Drawn, draw, draw_with, fast, inputs_with, lane, world};
use indexmap::IndexMap;
use serde_json::{Value, json};
use song::Cache;
use song::graph::{BUDGET, Handed, load_graph, web_graph};
use song::routing::{Chain, UseStep};

const SURGE: &str = "lib/k7-core/modules/audio/surge";
const MODULE: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\nparams({ Depth: 1 })\n";

/// A panel of `n` knobs, each bound to a param of the Chorus.
pub fn panel(n: usize) -> String {
    let widgets: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "w{i}: {{ type: \"knob\", col: {}, row: {}, bind: \"P{i}\", label: \"P{i}\" }}",
                i % 30,
                i / 30
            )
        })
        .collect();
    format!(
        "cols(30)\nrows({})\nwidgets({{ {} }})\n",
        n / 30 + 1,
        widgets.join(", ")
    )
}

/// A panel of `n` knobs whose labels are `label` bytes long: the same entries, a heavier output.
pub fn heavy_panel(n: usize, label: usize) -> String {
    let text = "l".repeat(label);
    let widgets: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "w{i}: {{ type: \"knob\", col: {}, row: {}, bind: \"P{i}\", label: \"{text}\" }}",
                i % 30,
                i / 30
            )
        })
        .collect();
    format!(
        "cols(30)\nrows({})\nwidgets({{ {} }})\n",
        n / 30 + 1,
        widgets.join(", ")
    )
}

/// A script of about `bytes` bytes.
pub fn script(bytes: usize) -> String {
    "// a line of a panel's script\n".repeat(bytes / 30 + 1)
}

fn steps(module: &str, n: usize) -> Chain {
    Chain {
        from: None,
        from_port: None,
        use_steps: (0..n)
            .map(|_| UseStep {
                module: module.to_owned(),
                inputs: None,
                enabled: None,
            })
            .collect(),
        out: vec![song::routing::Out {
            to: "OUT".into(),
            gain: 1.0,
            from_port: None,
        }],
    }
}

fn report(what: &str, d: &Drawn) {
    eprintln!("{what}: spent {} of {BUDGET}, capped {}", d.spent, d.capped);
}

// 200 lanes of 8 steps, each a 60-widget panel with an 8 KB script and 20 params, 200 clips a lane, 50 curves: no cap, a real spend.
#[test]
fn a_big_real_project_is_not_capped_and_its_spend_is_measured() {
    let mut owned: Vec<(String, String)> = vec![
        (
            format!("{SURGE}/module.k7x"),
            format!(
                "{MODULE}params({{ {} }})\n",
                (0..20).map(|i| format!("Q{i}: {i}")).collect::<Vec<_>>().join(", ")
            ),
        ),
        (format!("{SURGE}/ui.k7x"), panel(60)),
        (format!("{SURGE}/widgets.js"), script(8 * 1024)),
        ("clips/c.k7x".into(), "source(\"clips/c.wav\")\n".into()),
    ];
    let events: Vec<String> = (0..200).map(|i| format!("ref(\"c\").start(\"{}:1\")", i + 1)).collect();
    let l = lane(&format!("events([{}])\n", events.join(", ")));
    let mut graph = Vec::new();
    for n in 0..200 {
        owned.push((format!("nodes/lane{n}.k7x"), l.clone()));
        let mut run = Vec::new();
        for s in 0..8 {
            owned.push((
                format!("nodes/n{n}_{s}.k7x"),
                "extends(\"@k7-core/audio/surge\")\n".into(),
            ));
            run.push(format!("\"n{n}_{s}\""));
        }
        let first = run.remove(0);
        graph.push(format!("lane{n}: [{first}]"));
        let mut prev = first;
        for r in run {
            graph.push(format!("{prev}: [{r}]"));
            prev = r;
        }
        graph.push(format!("{prev}: [\"OUT\"]"));
    }
    for n in 0..50 {
        owned.push((
            format!("curves/k{n}.k7x"),
            "type(\"lfo\")\nshape(\"sine\")\nfrequency(1)\n".into(),
        ));
    }
    owned.push(("graph.k7x".into(), format!("modules({{ {} }})\n", graph.join(", "))));
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("a big project", 5, || draw(&files));
    report("200 lanes of 8 Chorus steps", &d);
    assert!(!d.capped && d.spent > 100_000 && d.spent < BUDGET, "{}", d.spent);
    assert_eq!(d.json["tracks"].as_array().map(Vec::len), Some(200));
    assert_eq!(d.json["curves"].as_object().map(serde_json::Map::len), Some(50));
}

// Twenty Surge-sized panels (900 widgets, 14 KB script) are not capped.
#[test]
fn twenty_surge_sized_panels_are_not_capped() {
    let mut owned: Vec<(String, String)> = vec![
        (format!("{SURGE}/module.k7x"), MODULE.to_owned()),
        (format!("{SURGE}/ui.k7x"), panel(900)),
        (format!("{SURGE}/widgets.js"), script(14 * 1024)),
    ];
    let mut graph = Vec::new();
    for n in 0..20 {
        owned.push((format!("nodes/s{n}.k7x"), "extends(\"@k7-core/audio/surge\")\n".into()));
        graph.push(format!("s{n}: [\"OUT\"]"));
    }
    owned.push(("graph.k7x".into(), format!("modules({{ {} }})\n", graph.join(", "))));
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("twenty surges", 5, || draw(&files));
    report("20 Surge-sized panels", &d);
    assert!(!d.capped);
    assert!(d.spent > 20 * 300, "{}", d.spent);
}

/// The graph of a Surge-sized module with a chain of `n` steps of it put in by hand, drawn.
fn surge_run(script_bytes: usize, n: usize, chain: &str) -> (Drawn, usize) {
    let files = [
        (format!("{SURGE}/module.k7x"), MODULE.to_owned()),
        (format!("{SURGE}/ui.k7x"), panel(900)),
        (format!("{SURGE}/widgets.js"), script(script_bytes)),
        ("nodes/surge.k7x".into(), "extends(\"@k7-core/audio/surge\")\n".into()),
        ("graph.k7x".into(), "modules({ surge: [\"OUT\"] })\n".into()),
    ];
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (src, roots) = world(&refs);
    let inputs = inputs_with(&src, &roots, &json!({}));
    let mut g = load_graph(&src, &mut Cache::new(), &roots, &inputs).expect("graph");
    g.chains = IndexMap::from([(chain.to_owned(), steps("surge", n))]);
    let a = fast("run", 5, || {
        web_graph(&src, &mut Cache::new(), &roots, &inputs, &g, &Handed::default()).expect("graph")
    });
    let full = a.graph.chains[chain]
        .use_steps
        .iter()
        .filter(|s| s.ui.is_some())
        .count();
    assert_eq!(a.graph.chains[chain].use_steps.len(), n, "the topology stays whole");
    let d = Drawn {
        json: Value::Null,
        findings: a.findings,
        capped: a.capped,
        spent: a.spent,
    };
    (d, full)
}

// A Surge-sized panel stepped 100,000 times is built once, paid per use, capped by weight; the topology is whole; one finding.
#[test]
fn a_surge_stepped_a_hundred_thousand_times_is_capped_by_weight() {
    let (d, full) = surge_run(14 * 1024, 100_000, "run");
    assert!(d.capped && full > 0 && full < 1_000, "{full} steps paid for");
    assert_eq!(
        d.findings
            .iter()
            .filter(|f| f.message.contains("units of output"))
            .count(),
        1
    );
}

// A widgets.js of 1 MiB stepped 1,000 times is capped by bytes.
#[test]
fn a_script_of_a_megabyte_stepped_a_thousand_times_is_capped_by_bytes() {
    let (d, full) = surge_run((1 << 20) - 64, 1_000, "run");
    assert!(d.capped && full > 0 && full < 100, "{full} steps paid for");
}

// A chain name of 1 MB over 100,000 steps: the engine address carries the name per use, so it is charged per use.
#[test]
fn a_pluginid_of_a_megabyte_per_step_is_charged_per_use() {
    let big = "c".repeat(1 << 20);
    let (d, full) = surge_run(1024, 100_000, &big);
    assert!(d.capped && full < 200, "{full} steps paid for");
}

// 400 distinct modules, each a 900-widget panel of 200-byte labels: capped, one finding. The resolve budget lets in about 990 such modules; past it the graph is refused.
#[test]
fn distinct_modules_with_big_panels_are_capped_with_one_finding() {
    let mut owned: Vec<(String, String)> = vec![
        (format!("{SURGE}/module.k7x"), MODULE.to_owned()),
        (format!("{SURGE}/ui.k7x"), heavy_panel(900, 200)),
    ];
    let mut graph = Vec::new();
    for n in 0..400 {
        owned.push((format!("nodes/m{n}.k7x"), "extends(\"@k7-core/audio/surge\")\n".into()));
        graph.push(format!("m{n}: [\"OUT\"]"));
    }
    owned.push(("graph.k7x".into(), format!("modules({{ {} }})\n", graph.join(", "))));
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("distinct", 5, || draw(&files));
    report("400 distinct 900-widget modules", &d);
    assert!(d.capped);
    assert_eq!(
        d.findings
            .iter()
            .filter(|f| f.message.contains("units of output"))
            .count(),
        1
    );
    let chains = d.json["chains"].as_object().expect("chains");
    assert_eq!(chains.len(), 400, "every chain keeps its place");
    let bare = chains.values().filter(|c| c["use"][0].get("ui").is_none()).count();
    assert!(bare > 0 && bare < 400);
}

// Past about a thousand such modules the RESOLVE budget (entries, not bytes) stops first: the graph is refused, not capped with its topology whole.
#[test]
fn past_the_resolve_budget_the_graph_is_refused_not_capped() {
    let mut owned: Vec<(String, String)> = vec![
        (format!("{SURGE}/module.k7x"), MODULE.to_owned()),
        (format!("{SURGE}/ui.k7x"), heavy_panel(900, 200)),
    ];
    let mut graph = Vec::new();
    for n in 0..1_000 {
        owned.push((format!("nodes/m{n}.k7x"), "extends(\"@k7-core/audio/surge\")\n".into()));
        graph.push(format!("m{n}: [\"OUT\"]"));
    }
    owned.push(("graph.k7x".into(), format!("modules({{ {} }})\n", graph.join(", "))));
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (src, roots) = world(&files);
    let answer = fast("refused", 5, || {
        song::graph::load_web_graph(
            &src,
            &mut Cache::new(),
            &roots,
            song::lanes::rig::Rig::default(),
            &Handed::default(),
        )
    });
    let Err(song::graph::GraphError::Refused(r)) = answer else {
        panic!("the graph was drawn");
    };
    assert!(
        r.findings.iter().all(|f| f.file.starts_with("nodes/m")),
        "{:?}",
        r.findings
    );
}

// A panel of 90,000 widgets looks its defaults up once: the bound set is built once per module.
#[test]
fn a_defaults_lookup_over_a_huge_panel_is_one_pass() {
    let owned = [
        (format!("{SURGE}/module.k7x"), MODULE.to_owned()),
        (format!("{SURGE}/ui.k7x"), panel(90_000)),
        (
            "nodes/c.k7x".to_owned(),
            "extends(\"@k7-core/audio/surge\")\n".to_owned(),
        ),
        ("graph.k7x".to_owned(), "modules({ c: [\"OUT\"] })\n".to_owned()),
    ];
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("defaults", 5, || draw_with(&refs, &json!({}), &Handed::default()));
    let ui = d.json.pointer("/chains/c/use/0/ui/widgets").and_then(Value::as_object);
    assert!(
        ui.is_some_and(|w| w.len() == 90_000) || d.capped,
        "the panel is drawn whole or the step is bare"
    );
}

/// A project of one lane `m` holding `events`, drawn with `handed`.
fn lane_drawn(events: &str, handed: &Handed, extra: &[(&str, &str)]) -> Drawn {
    let l = lane(&format!("events([{events}])\n"));
    let mut files: Vec<(&str, &str)> = vec![
        ("clips/a.k7x", "source(\"clips/a.wav\")\n"),
        ("nodes/m.k7x", l.as_str()),
        ("graph.k7x", "modules({ m: [\"OUT\"] })\n"),
    ];
    files.extend_from_slice(extra);
    fast("lane", 5, || draw_with(&files, &json!({}), handed))
}

// trackClips: 100,000 clips at one start and 100,000 extents at one start give one map, the last winning, and a clip each.
#[test]
fn a_hundred_thousand_clips_and_extents_at_one_start_are_one_map() {
    let events = vec!["ref(\"a\").start(\"1:1\")"; 100_000].join(", ");
    let handed = Handed {
        extents: Some(
            [(
                "m".to_owned(),
                (0..100_000)
                    .map(|n| song::graph::ClipExtent {
                        start: 0.0,
                        length: f64::from(n) + 1.0,
                    })
                    .collect(),
            )]
            .into_iter()
            .collect(),
        ),
        ..Handed::default()
    };
    let d = lane_drawn(&events, &handed, &[]);
    let clips = d
        .json
        .pointer("/trackClips/m")
        .and_then(Value::as_array)
        .expect("clips");
    assert_eq!(clips.len(), 100_000);
    assert!(clips.iter().all(|c| c["len"] == json!(100_000.0)));
    assert!(!d.capped);
}

// 417: a start no position can spell keeps its place with `tc: ""`; the others are spelled.
#[test]
fn a_start_with_no_spelling_keeps_its_place_with_an_empty_position() {
    let d = lane_drawn(
        "ref(\"a\").start(1e300), ref(\"a\").start(\"2:1\")",
        &Handed::default(),
        &[],
    );
    let clips = d
        .json
        .pointer("/trackClips/m")
        .and_then(Value::as_array)
        .expect("clips");
    assert_eq!(clips.len(), 2);
    assert_eq!(
        (clips[0]["tc"].as_str(), clips[1]["tc"].as_str()),
        (Some("2:1"), Some(""))
    );
}

// A ref of 1 MB on 20 clips is charged for its bytes: 16,000 units each.
#[test]
fn a_megabyte_ref_on_a_clip_is_charged_by_its_bytes() {
    let big = "r".repeat(1 << 20);
    let events: Vec<String> = (0..20)
        .map(|n| format!("ref(\"{big}\").start(\"{}:1\")", n + 1))
        .collect();
    let d = lane_drawn(&events.join(", "), &Handed::default(), &[]);
    assert!(d.spent > 20 * 16_000, "{}", d.spent);
}

// Handed: ten plans of 100,000 names are stopped by the budget, whole when they fit; a name of 1 MB is charged.
#[test]
fn handed_plans_are_charged_by_their_names_and_stopped_by_the_budget() {
    let names: Vec<String> = (0..100_000).map(|n| format!("n{n}")).collect();
    let plan = |at: &str| song::graph::FreezeMark {
        at: at.to_owned(),
        lanes: names.clone(),
        modules: Vec::new(),
    };
    let handed = Handed {
        manual: (0..30).map(|n| plan(&format!("p{n}"))).collect(),
        ..Handed::default()
    };
    let d = fast("handed", 5, || {
        draw_with(&[("graph.k7x", "modules({ a: [\"OUT\"] })\n")], &json!({}), &handed)
    });
    assert!(d.capped);
    let frozen = d
        .json
        .pointer("/frozen")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    assert!(
        frozen > 100_000 && frozen <= 100_030,
        "the same 100,000 names are one map: {frozen}"
    );
    let big = Handed {
        manual: vec![song::graph::FreezeMark {
            at: "x".repeat(1 << 20),
            lanes: Vec::new(),
            modules: Vec::new(),
        }],
        ..Handed::default()
    };
    let d = draw_with(&[("graph.k7x", "modules({ a: [\"OUT\"] })\n")], &json!({}), &big);
    assert!(d.spent > 16_000, "{}", d.spent);
}

// Curves: a curve of 200,000 points is charged twice over (the points and the definition); 100,000 curves of one point are in time.
#[test]
fn curves_are_charged_by_what_they_serialize_to() {
    let points: Vec<String> = (0..200_000)
        .map(|n| format!("{{ time: \"{}:1\", value: 1 }}", n + 1))
        .collect();
    let big = format!("type(\"custom\")\npoints([{}])\n", points.join(", "));
    let d = lane_drawn(
        "ref(\"a\").start(\"1:1\")",
        &Handed::default(),
        &[("curves/big.k7x", big.as_str())],
    );
    assert!(d.spent > 2 * 200_000 * 20 / 64, "{}", d.spent);
}

#[test]
fn a_hundred_thousand_curves_of_one_point_are_in_time() {
    let owned: Vec<(String, String)> = (0..100_000)
        .map(|n| {
            (
                format!("curves/c{n}.k7x"),
                "type(\"onoff\")\npoints([{ time: \"1:1\", value: 1 }])\n".to_owned(),
            )
        })
        .collect();
    let mut files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    files.push(("graph.k7x", "modules({ a: [\"OUT\"] })\n"));
    let d = fast("curves", 5, || draw(&files));
    assert!(
        d.capped
            || d.json
                .pointer("/curves")
                .and_then(Value::as_object)
                .is_some_and(|c| c.len() == 100_000)
    );
}

// 419: a lane's alternatives are output, charged by their bytes per lane; 200 lanes of one prototype with a 1 MB alternative cap the answer.
#[test]
fn lanes_sharing_a_long_alternative_name_are_charged_per_lane() {
    let big = "a".repeat(1 << 20);
    let (src, roots) = world(&[
        (
            "nodes/base.k7x",
            format!("extends(\"@meta/track\")\nalternatives([{{ name: \"{big}\" }}])\n").as_str(),
        ),
        ("graph.k7x", "modules({ base: [\"OUT\"] })\n"),
    ]);
    let mut i = inputs_with(&src, &roots, &json!({}));
    let proto = i.local.modules.get("base").expect("base").clone();
    let mut modules: IndexMap<String, Vec<String>> = IndexMap::new();
    for n in 0..200 {
        i.local.modules.insert(format!("l{n}"), proto.clone());
        modules.insert(format!("l{n}"), vec!["OUT".into()]);
    }
    i.routing.segments.modules = Some(modules);
    let g = load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph");
    let a = fast("alternatives", 5, || {
        web_graph(&src, &mut Cache::new(), &roots, &i, &g, &Handed::default()).expect("graph")
    });
    assert!(a.capped, "spent {}", a.spent);
    let lanes = a.graph.track_alternatives.as_ref().map_or(0, IndexMap::len);
    assert!(lanes > 0 && lanes < 200, "{lanes} lanes");
    let bytes: usize = a
        .graph
        .track_alternatives
        .iter()
        .flatten()
        .map(|(_, c)| c.alternatives.iter().map(String::len).sum::<usize>())
        .sum();
    assert!(bytes < 70 << 20, "{bytes} bytes of alternatives");
}
