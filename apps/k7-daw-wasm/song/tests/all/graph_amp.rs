// graph_amp.rs — what an amp model file is, read by the five keys the engine reads (425): ampModel.test.ts, modelCache.test.ts and the hostile rows.
// A reader would get wrong that a `.nam` is never opened and a `.json` is read once per call, into a probe, never a Value.

use crate::graph_support;

use graph_support::{Drawn, draw, fast, inputs_with, world};
use indexmap::IndexMap;
use serde_json::{Value, json};
use song::graph::view::{AmpKind, amp_of_json};
use song::graph::{Handed, load_graph, web_graph};
use song::routing::{Chain, Out, UseStep};
use song::{Cache, Entry, MemSource, Source, SourceError, Span, Stat};
use std::cell::Cell;

fn of(json: &str) -> Option<(AmpKind, u32)> {
    amp_of_json(json.as_bytes()).map(|a| (a.kind, a.knobs))
}

// ampModel.test.ts "counts an AIDA-X net's inputs past the sound", "reads a Proteus net", "caps at the three knobs", "refuses a json that is neither".
#[test]
fn a_json_model_is_aida_x_by_layers_and_in_shape_or_proteus_by_model_data_and_state_dict() {
    assert_eq!(of(r#"{"layers":[],"in_shape":[null,1,2]}"#), Some((AmpKind::AidaX, 1)));
    assert_eq!(of(r#"{"layers":[],"in_shape":[1]}"#), Some((AmpKind::AidaX, 0)));
    assert_eq!(of(r#"{"model_data":{},"state_dict":{}}"#), Some((AmpKind::Proteus, 0)));
    assert_eq!(
        of(r#"{"model_data":{"input_size":3},"state_dict":null}"#),
        Some((AmpKind::Proteus, 2))
    );
    assert_eq!(of(r#"{"layers":[],"in_shape":[9]}"#), Some((AmpKind::AidaX, 3)));
    assert_eq!(of(r#"{"foo":1}"#), None);
    assert_eq!(
        of(r#"{"model_data":{"input_size":3}}"#),
        None,
        "a Proteus net names its state_dict"
    );
}

// The hostile rows: `in_shape` of nothing, a huge number, text, a negative; `model_data` a number; not UTF-8; a nest; a key of 1 MB.
#[test]
fn a_hostile_json_model_answers_none_or_a_clamped_count() {
    assert_eq!(of(r#"{"layers":[],"in_shape":[]}"#), None);
    assert_eq!(of(r#"{"layers":[],"in_shape":[1e300]}"#), Some((AmpKind::AidaX, 3)));
    assert_eq!(of(r#"{"layers":[],"in_shape":["x"]}"#), None);
    assert_eq!(of(r#"{"layers":[],"in_shape":[-3]}"#), Some((AmpKind::AidaX, 0)));
    assert_eq!(of(r#"{"model_data":1,"state_dict":{}}"#), None);
    assert_eq!(amp_of_json(&[0xff, 0xfe, b'{', b'}']), None);
    let nest = format!("{}{}", "[".repeat(300), "]".repeat(300));
    assert_eq!(of(&nest), None);
    let key = "k".repeat(1 << 20);
    assert_eq!(
        of(&format!(r#"{{"{key}":1,"layers":[],"in_shape":[2]}}"#)),
        Some((AmpKind::AidaX, 1))
    );
}

fn model_project(path: &str, body: &str) -> Vec<(String, String)> {
    vec![
        (
            "nodes/amp.k7x".into(),
            format!("plugin({{ type: \"internal\", name: \"Amp\" }})\nparams({{ model: \"{path}\" }})\n"),
        ),
        (format!("nodes/assets/{path}"), body.to_owned()),
        ("graph.k7x".into(), "modules({ amp: [\"OUT\"] })\n".into()),
    ]
}

fn drawn(path: &str, body: &str) -> Drawn {
    let owned = model_project(path, body);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    draw(&refs)
}

// ampModel.test.ts "knows a .nam without opening it, and reads a .json off disk"; the extensions in any case.
#[test]
fn a_nam_is_known_by_its_name_and_a_json_is_read_through_the_source() {
    let nam = drawn("amp.NAM", "not a model at all");
    assert_eq!(
        nam.json.pointer("/chains/amp/use/0/ampModel"),
        Some(&json!({"kind": "nam", "knobs": 0}))
    );
    let aida = drawn("net.JSON", r#"{"layers":[],"in_shape":[1,3]}"#);
    assert_eq!(
        aida.json.pointer("/chains/amp/use/0/ampModel"),
        Some(&json!({"kind": "aida-x", "knobs": 2}))
    );
    let none = drawn("net.json", r#"{"nothing":true}"#);
    assert!(none.json.pointer("/chains/amp/use/0/ampModel").is_none());
}

// A .json over 4 MiB is not read, and the step still draws.
#[test]
fn a_model_over_four_mebibytes_is_not_read() {
    let big = format!(r#"{{"layers":[],"in_shape":[2],"pad":"{}"}}"#, "x".repeat(5 << 20));
    let d = fast("5 MiB", 5, || drawn("big.json", &big));
    assert!(d.json.pointer("/chains/amp/use/0/ampModel").is_none());
    assert!(d.json.pointer("/chains/amp/use/0").is_some());
}

// modelCache.test.ts: a missing model names its file, and nothing else is offered once it resolves.
#[test]
fn a_model_that_is_a_folder_named_like_a_json_is_none() {
    let files = [
        (
            "nodes/amp.k7x",
            "plugin({ type: \"internal\", name: \"Amp\" })\nparams({ model: \"x.json\" })\n",
        ),
        ("nodes/assets/x.json/inner.txt", "x"),
        ("graph.k7x", "modules({ amp: [\"OUT\"] })\n"),
    ];
    let d = draw(&files);
    assert!(d.json.pointer("/chains/amp/use/0/ampModel").is_none());
}

struct Reads<'a> {
    inner: &'a MemSource,
    reads: Cell<usize>,
}

impl Source for Reads<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        if path.ends_with(".json") {
            self.reads.set(self.reads.get() + 1);
        }
        self.inner.read(path, span)
    }
}

// The brief's row: one model stepped 100,000 times is read once.
#[test]
fn one_model_stepped_a_hundred_thousand_times_is_read_once() {
    let owned = model_project("net.json", r#"{"layers":[],"in_shape":[1,2]}"#);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (mem, roots) = world(&refs);
    let counting = Reads {
        inner: &mem,
        reads: Cell::new(0),
    };
    let inputs = inputs_with(&mem, &roots, &json!({}));
    let mut g = load_graph(&counting, &mut Cache::new(), &roots, &inputs).expect("graph");
    let steps = (0..100_000)
        .map(|_| UseStep {
            module: "amp".into(),
            inputs: None,
            enabled: None,
        })
        .collect();
    g.chains = IndexMap::from([(
        "run".to_owned(),
        Chain {
            from: None,
            from_port: None,
            use_steps: steps,
            out: vec![Out {
                to: "OUT".into(),
                gain: 1.0,
                from_port: None,
            }],
        },
    )]);
    let reads_before = counting.reads.get();
    let a = fast("model", 5, || {
        web_graph(&counting, &mut Cache::new(), &roots, &inputs, &g, &Handed::default()).expect("graph")
    });
    assert!(
        counting.reads.get() - reads_before <= 1,
        "{}",
        counting.reads.get() - reads_before
    );
    assert!(a.graph.chains["run"].use_steps.iter().any(|s| s.amp_model.is_some()));
}

#[test]
fn the_json_of_an_answer_is_free_of_nulls() {
    let d = drawn("net.json", r#"{"layers":[],"in_shape":[1,3]}"#);
    let mut bad = Vec::new();
    graph_support::no_null(&d.json, "", &mut bad);
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(
        d.json
            .pointer("/chains/amp/use/0/ampModel/knobs")
            .and_then(Value::as_u64),
        Some(2)
    );
}

// ampModel.ts `ampModelOfJson`: a sequence is no object, a null `input_size` is an absent one, the last of a repeated key stands.
#[test]
fn the_probe_reads_a_file_as_the_typescript_reads_its_parse() {
    assert_eq!(of("[[1],[1,2,3]]"), None);
    assert_eq!(of(r#"[[],[1,2],{}]"#), None);
    assert_eq!(
        of(r#"{"model_data":{"input_size":null},"state_dict":{}}"#),
        Some((AmpKind::Proteus, 0))
    );
    assert_eq!(of(r#"{"model_data":{"input_size":"x"},"state_dict":{}}"#), None);
    assert_eq!(of(r#"{"layers":[],"in_shape":[null]}"#), None);
    assert_eq!(
        of(r#"{"layers":0,"layers":[],"in_shape":[1,2]}"#),
        Some((AmpKind::AidaX, 1))
    );
    assert_eq!(
        of(r#"{"layers":[],"in_shape":[1,9],"in_shape":[1,2]}"#),
        Some((AmpKind::AidaX, 1))
    );
    assert_eq!(of(r#"{"layers":[],"in_shape":[1,2],"layers":3}"#), None);
}
