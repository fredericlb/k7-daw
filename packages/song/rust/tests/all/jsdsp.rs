// jsdsp.rs — a jsdsp module's graph: the states of decision 350, the validator of 351, the Source's stamps of 352 (jsdspBuildFailure.test.ts).
// A reader would get wrong that nothing is built: a graph that cannot play is a state and a pass-through, never a throw or a write.

use crate::freeze_support;
use crate::support;

use freeze_support::{Counting, fast};
use serde_json::json;
use song::jsdsp::{GraphState, MAX_GRAPH, NODE_TYPES, PASS_THROUGH_GRAPH, cache_of, jsdsp_graph, validate_graph};
use song::record::Recorder;
use song::{BundleFile, DiskSource, MemSource, Source, SourceBundle, SourceError};
use std::collections::BTreeMap;
use support::{Tmp, spell};

const GOOD: &str = "{\"params\":[{\"name\":\"Cutoff\",\"min\":0,\"max\":1,\"default\":0.5}],\"nodes\":[{\"type\":\"mul\",\"in\":[\"Cutoff\",2],\"out\":\"x\"}]}\n";
const OLD_GRAPH: &str = "{\"params\":[{\"name\":\"Cutoff\",\"min\":0,\"max\":1}],\"nodes\":[]}\n";

fn world(t: &Tmp) -> DiskSource {
    DiskSource::new(&[spell(&t.0)]).expect("a source")
}

fn older(t: &Tmp, rel: &str, secs: u64) {
    let f = std::fs::File::options().write(true).open(t.0.join(rel)).expect("open");
    f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(secs))
        .expect("mtime");
}

fn state(t: &Tmp) -> (GraphState, String) {
    let g = jsdsp_graph(&world(t), &t.at("m/jsdsp.ts")).expect("a source");
    (g.state, g.text.to_string())
}

#[test]
fn the_cache_of_a_source_drops_its_extension_and_takes_json() {
    // decision 349
    assert_eq!(cache_of("/m/jsdsp.ts"), "/m/jsdsp.json");
    assert_eq!(cache_of("/m/jsdsp.tsx"), "/m/jsdsp.json");
    assert_eq!(cache_of("/m/a.js"), "/m/a.json");
    assert_eq!(cache_of("/m/a.jsx"), "/m/a.json");
    assert_eq!(cache_of("/m/a.d.ts"), "/m/a.d.json");
    assert_eq!(cache_of("/m/noext"), "/m/noext.json");
}

#[test]
fn a_legacy_graph_beside_its_source_plays_with_no_word_and_is_never_rebuilt() {
    // nothing builds a jsdsp.ts any more: its committed cache is the graph, newer or older than the source
    let t = Tmp::new("js-legacy");
    t.write("m/jsdsp.ts", "export default {};\n");
    t.write("m/jsdsp.json", OLD_GRAPH);
    older(&t, "m/jsdsp.json", 3600);
    assert_eq!(state(&t), (GraphState::Orphan, OLD_GRAPH.to_owned()));
}

/// A Source of two files, whose graph file cannot be read as a bundle's non-UTF-8 one cannot.
struct Stamped {
    read_missing: bool,
}

impl Source for Stamped {
    fn stat(&self, _path: &str) -> Result<song::Stat, SourceError> {
        Ok(song::Stat {
            kind: song::Kind::File,
            len: 4,
            stamp: song::Stamp {
                mtime_ns: 1,
                len: 4,
                settled: true,
            },
        })
    }

    fn list(&self, _dir: &str) -> Result<Vec<song::Entry>, SourceError> {
        Err(SourceError::NotFound)
    }

    fn read(&self, path: &str, _span: song::Span) -> Result<Vec<u8>, SourceError> {
        if self.read_missing {
            return Err(SourceError::Missing);
        }
        assert!(path.ends_with(".json"));
        Ok(GOOD.as_bytes().to_vec())
    }

    fn stamps_are_times(&self) -> bool {
        true
    }
}

#[test]
fn a_graph_the_bundle_stats_and_cannot_read_is_not_utf8_and_not_a_missing_file() {
    // decision 344's reading of a stat'ed file the bundle cannot read, for the graph too
    let src = Stamped { read_missing: true };
    let g = jsdsp_graph(&src, "/p/m/jsdsp.ts").expect("an answer, not needs_files");
    assert!(matches!(g.state, GraphState::Invalid(_)), "{:?}", g.state);
    assert_eq!(&*g.text, PASS_THROUGH_GRAPH);
}

#[test]
fn a_memory_source_reads_a_legacy_graph_like_a_disk() {
    // decision 352: a memory stamp is a hash, so nothing is ever compared with it
    let mem = |files: &[(&str, &str)]| {
        let mut held = BTreeMap::new();
        let mut names = Vec::new();
        for (n, t) in files {
            held.insert(
                format!("/p/m/{n}"),
                BundleFile {
                    text: Some((*t).to_owned()),
                    len: None,
                    ..Default::default()
                },
            );
            names.push(song::Entry {
                name: (*n).to_owned(),
                kind: song::Kind::File,
            });
        }
        MemSource::from_bundle(SourceBundle {
            format: 1,
            mounts: vec!["/p".into()],
            files: held,
            lists: BTreeMap::from([("/p/m".to_owned(), names)]),
            ..Default::default()
        })
        .expect("bundle")
    };
    let src = mem(&[("jsdsp.ts", "x"), ("jsdsp.json", GOOD)]);
    assert!(!src.stamps_are_times());
    let g = jsdsp_graph(&src, "/p/m/jsdsp.ts").expect("a source");
    assert_eq!((g.state, &*g.text), (GraphState::Orphan, GOOD));
    assert_eq!(g.json_path, "/p/m/jsdsp.json");
    assert!(world(&Tmp::new("js-times")).stamps_are_times());
}

#[test]
fn a_recorder_and_a_watch_forward_the_kind_of_their_stamps() {
    // decision 352
    let t = Tmp::new("js-forward");
    let disk = world(&t);
    assert!(Recorder::new(&disk).stamps_are_times());
    assert!(song::Watch::new(&disk).stamps_are_times());
}

#[test]
fn a_source_with_no_graph_is_missing_and_plays_the_pass_through() {
    // jsdspBuildFailure.test.ts "is reported when the source is missing" and "sends a pass-through graph"; decision 350
    let t = Tmp::new("js-missing");
    t.mkdir("m");
    assert_eq!(state(&t), (GraphState::Missing, PASS_THROUGH_GRAPH.to_owned()));
    t.write("m/jsdsp.ts", "export default {};\n");
    assert_eq!(state(&t), (GraphState::Missing, PASS_THROUGH_GRAPH.to_owned()));
}

#[test]
fn a_graph_with_no_source_is_an_orphan_and_plays_with_no_word() {
    // decision 350
    let t = Tmp::new("js-orphan");
    t.write("m/jsdsp.json", GOOD);
    assert_eq!(state(&t), (GraphState::Orphan, GOOD.to_owned()));
}

fn literal(t: &Tmp) -> (GraphState, String) {
    let g = jsdsp_graph(&world(t), &t.at("m/dsp.k7x")).expect("a source");
    (g.state, g.text.to_string())
}

#[test]
fn a_dsp_k7x_is_decoded_on_every_read_and_needs_no_cache() {
    // docs/dsp-literal-plan.md: no cache, so no staleness rule
    let t = Tmp::new("js-literal");
    t.write(
        "m/dsp.k7x",
        "params({ Mix: { min: 0, max: 1, default: 0.5 } })\ngraph([mix(\"Mix\")])\n",
    );
    let (state, text) = literal(&t);
    assert_eq!(state, GraphState::Fresh);
    assert!(text.contains("\"type\": \"mix\""), "{text}");
    t.write("m/dsp.k7x", "graph([])\n");
    assert_eq!(
        literal(&t),
        (
            GraphState::Fresh,
            "{\n  \"params\": [],\n  \"nodes\": []\n}\n".to_owned()
        )
    );
}

#[test]
fn a_dsp_k7x_that_does_not_decode_is_invalid_with_its_line_and_plays_the_pass_through() {
    let t = Tmp::new("js-literal-bad");
    t.write("m/dsp.k7x", "params({})\ngraph([\n  output(amul(\"dyr\", 1)),\n])\n");
    let (state, text) = literal(&t);
    let GraphState::Invalid(why) = state else {
        panic!("{state:?}")
    };
    assert!(
        why.starts_with("line 3: ") && why.contains("`dyr` names nothing"),
        "{why}"
    );
    assert_eq!(text, PASS_THROUGH_GRAPH);
}

#[test]
fn a_dsp_k7x_that_is_not_there_is_missing() {
    let t = Tmp::new("js-literal-none");
    t.mkdir("m");
    assert_eq!(literal(&t), (GraphState::Missing, PASS_THROUGH_GRAPH.to_owned()));
}

fn invalid(t: &Tmp) {
    let (s, text) = state(t);
    assert!(matches!(s, GraphState::Invalid(_)), "{s:?}");
    assert_eq!(text, PASS_THROUGH_GRAPH);
}

#[test]
fn a_graph_the_engine_cannot_read_is_invalid_and_plays_the_pass_through() {
    // jsdsp row "invalid JSON"; decision 350
    let t = Tmp::new("js-invalid");
    t.write("m/jsdsp.ts", "x");
    t.write("m/jsdsp.json", "{ nope");
    invalid(&t);
}

#[test]
fn a_graph_of_two_mebibytes_is_invalid_and_never_read() {
    // jsdsp row "2 MiB"; decision 350
    let t = Tmp::new("js-big");
    t.write("m/jsdsp.ts", "x");
    t.write(
        "m/jsdsp.json",
        &format!("{{\"nodes\":[],\"pad\":\"{}\"}}", "a".repeat(2 * 1024 * 1024)),
    );
    let disk = world(&t);
    let src = Counting::new(&disk);
    let g = jsdsp_graph(&src, &t.at("m/jsdsp.ts")).expect("a source");
    assert!(matches!(g.state, GraphState::Invalid(_)));
    assert_eq!(src.reads(), 0);
}

#[test]
fn a_graph_nested_forty_deep_is_invalid() {
    // jsdsp row "depth 40"; decision 351
    let t = Tmp::new("js-deep");
    t.write("m/jsdsp.ts", "x");
    t.write(
        "m/jsdsp.json",
        &format!("{{\"nodes\":[],\"x\":{}1{}}}", "[".repeat(40), "]".repeat(40)),
    );
    invalid(&t);
}

#[test]
fn a_graph_that_is_not_utf8_is_invalid() {
    // jsdsp row "not UTF-8"; decision 350
    let t = Tmp::new("js-utf8");
    t.write("m/jsdsp.ts", "x");
    std::fs::write(t.0.join("m/jsdsp.json"), [0xff, 0xfe, b'{', b'}']).expect("write");
    invalid(&t);
}

#[test]
fn a_graph_that_is_a_folder_is_invalid() {
    // jsdsp row "a folder"; decision 350
    let t = Tmp::new("js-folder");
    t.write("m/jsdsp.ts", "x");
    t.mkdir("m/jsdsp.json");
    invalid(&t);
}

#[cfg(unix)]
#[test]
fn a_graph_that_is_a_link_out_of_the_mounts_is_invalid() {
    // jsdsp row "a link out"; decision 350
    let t = Tmp::new("js-link");
    let out = Tmp::new("js-link-out");
    out.write("g.json", GOOD);
    t.write("m/jsdsp.ts", "x");
    std::os::unix::fs::symlink(out.0.join("g.json"), t.0.join("m/jsdsp.json")).expect("link");
    invalid(&t);
}

#[test]
fn a_bundle_that_does_not_hold_the_graph_is_the_error() {
    // decision 350: Missing from a MemSource is Err
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: BTreeMap::new(),
        lists: BTreeMap::from([(
            "/p".to_owned(),
            vec![song::Entry {
                name: "m".into(),
                kind: song::Kind::Dir,
            }],
        )]),
        ..Default::default()
    };
    let src = MemSource::from_bundle(bundle).expect("bundle");
    assert_eq!(jsdsp_graph(&src, "/p/m/jsdsp.ts"), Err(SourceError::Missing));
}

#[test]
fn a_recorder_keeps_the_text_of_the_graph() {
    // read set: a graph is read whole, so a bundle made from the compile holds it
    let t = Tmp::new("js-record");
    t.write("m/jsdsp.ts", "x");
    t.write("m/jsdsp.json", GOOD);
    let disk = world(&t);
    let rec = Recorder::new(&disk);
    jsdsp_graph(&rec, &t.at("m/jsdsp.ts")).expect("a source");
    let bundle = rec.into_bundle(&[spell(&t.0)]);
    assert_eq!(bundle.files[&t.at("m/jsdsp.json")].text.as_deref(), Some(GOOD));
}

fn ok(v: serde_json::Value) {
    assert_eq!(validate_graph(&v.to_string()), Ok(()), "{v}");
}

fn bad(v: serde_json::Value) {
    assert!(validate_graph(&v.to_string()).is_err(), "{v}");
}

#[test]
fn the_validator_takes_what_the_engine_takes() {
    // decision 351: DspGraph::build tolerates all of these
    assert_eq!(validate_graph(PASS_THROUGH_GRAPH), Ok(()));
    assert_eq!(validate_graph(OLD_GRAPH), Ok(()));
    assert_eq!(validate_graph(GOOD), Ok(()));
    ok(json!({"nodes": []}));
    ok(json!({"params": [], "nodes": [], "future": {"anything": [1, 2]}}));
    ok(json!({"params": [{"name": "A", "labels": ["x", "y"], "extra": true}], "nodes": []}));
    ok(
        json!({"nodes": [{"type": "ain", "port": 1, "out": "w"}, {"type": "adelay", "maxMs": 20, "k": [1, "t"], "in": ["w"], "out": "v"}]}),
    );
    ok(json!({"nodes": [{"type": "gain", "in": [1, "x", 2.5]}]}));
    // params the engine skips or reads loosely
    ok(json!({"params": null, "nodes": []}));
    ok(json!({"params": 3, "nodes": []}));
    ok(json!({"params": "x", "nodes": []}));
    ok(json!({"params": {"name": "A"}, "nodes": []}));
    ok(json!({"params": [3, null, "x", [1], {"name": "A"}], "nodes": []}));
    ok(json!({"params": [{"name": 5}], "nodes": []}));
    ok(
        json!({"params": [{"name": true}, {"name": 5}, {"name": 2.5}, {"name": [1]}, {"name": {}}, {"name": {}}], "nodes": []}),
    );
    ok(json!({"params": [{"name": "A", "min": null, "max": "x", "default": [1]}], "nodes": []}));
    ok(
        json!({"params": [{"name": "A", "labels": "x"}, {"name": "B", "labels": [1, null, {"a": 1}, true]}], "nodes": []}),
    );
    ok(json!({"params": [{"name": "A", "labels": null}], "nodes": []}));
    // nodes the engine skips, and keys a type never reads
    ok(json!({"nodes": [3, null, "x", [1], {"type": "gain"}]}));
    ok(json!({"nodes": [3]}));
    ok(json!({"nodes": [{"type": "gain", "in": null, "out": null}]}));
    ok(json!({"nodes": [{"type": "gain", "in": [null, true, false, 1, "a", [1], {"a": 1}], "out": 3}]}));
    ok(json!({"nodes": [{"type": "gain", "in": {"a": 1}}, {"type": "gain", "in": "x"}, {"type": "gain", "out": [1]}]}));
    ok(json!({"nodes": [{"type": "adelay", "k": "x", "maxMs": null}, {"type": "adelay", "maxMs": "20"}]}));
    ok(json!({"nodes": [{"type": "ain", "port": "1"}, {"type": "ain", "port": null}, {"type": "ain", "port": true}]}));
    ok(json!({"nodes": [{"type": "alfo", "k": [null, true], "shape": 3, "mode": {}}, {"type": "biquad", "mode": 7}]}));
    ok(json!({"nodes": [{"type": "mul", "port": "x", "maxMs": "x", "k": 1, "shape": []}]}));
}

#[test]
fn the_validator_refuses_what_the_engine_cannot_read() {
    // decision 351: the errors of DspGraph::build, and no other
    bad(json!([]));
    bad(json!("graph"));
    bad(json!(3));
    bad(json!(null));
    bad(json!({}));
    bad(json!({"params": []}));
    bad(json!({"nodes": 3}));
    bad(json!({"nodes": null}));
    bad(json!({"nodes": {"a": 1}}));
    bad(json!({"params": [{"min": 0}], "nodes": []}));
    bad(json!({"params": [{"name": ""}], "nodes": []}));
    bad(json!({"params": [{"name": null}], "nodes": []}));
    bad(json!({"params": [{"name": "sr"}], "nodes": []}));
    bad(json!({"params": [{"name": "A"}, {"name": "A"}], "nodes": []}));
    // a name is compared as the engine prints it
    bad(json!({"params": [{"name": 5}, {"name": "5"}], "nodes": []}));
    bad(json!({"params": [{"name": true}, {"name": "1"}], "nodes": []}));
    bad(json!({"params": [{"name": [1]}, {"name": [2, 3]}], "nodes": []}));
    bad(json!({"nodes": [{}]}));
    bad(json!({"nodes": [{"type": null}]}));
    bad(json!({"nodes": [{"type": 5}]}));
    bad(json!({"nodes": [{"type": ["gain"]}]}));
    bad(json!({"nodes": [{"type": "warp"}]}));
    bad(json!({"nodes": [{"type": "Gain"}]}));
    bad(json!({"nodes": [{"type": "gain"}, {"type": "warp"}]}));
}

#[test]
fn a_number_past_the_range_is_refused() {
    // jsdsp row "1e999 as a number"; decision 351: JSON that is not a finite number is not JSON here
    assert!(validate_graph("{\"params\":[{\"name\":\"A\",\"max\":1e999}],\"nodes\":[]}").is_err());
    assert!(validate_graph("{\"nodes\":[{\"type\":\"ain\",\"port\":1e999}]}").is_err());
}

#[test]
fn a_duplicate_key_takes_the_last_as_the_parser_does() {
    // decision 351
    assert_eq!(validate_graph("{\"nodes\":3,\"nodes\":[]}"), Ok(()));
    assert!(validate_graph("{\"nodes\":[],\"nodes\":3}").is_err());
}

#[test]
fn a_message_never_holds_the_files_words() {
    // decision 351: a fixed sentence
    let text =
        "{\"params\":[{\"name\":\"SECRET-NAME\"},{\"name\":\"SECRET-NAME\"}],\"nodes\":[{\"type\":\"SECRET-TYPE\"}]}";
    let why = validate_graph(text).expect_err("refused");
    assert!(!why.contains("SECRET"), "{why}");
    let why = validate_graph("{\"nodes\":[{\"type\":\"SECRET-TYPE\"}]}").expect_err("refused");
    assert!(!why.contains("SECRET"), "{why}");
}

#[test]
fn the_graph_limit_is_a_mebibyte_of_text() {
    // decision 351
    assert_eq!(MAX_GRAPH, 1 << 20);
    let pad = "a".repeat(usize::try_from(MAX_GRAPH).expect("small") + 1);
    assert!(validate_graph(&format!("{{\"nodes\":[],\"p\":\"{pad}\"}}")).is_err());
}

#[test]
fn brackets_inside_a_string_do_not_count_as_depth() {
    // decision 351, as the engine's scan
    let s = format!("{{\"nodes\":[],\"p\":\"{}\"}}", "[".repeat(100));
    assert_eq!(validate_graph(&s), Ok(()));
    let esc = format!("{{\"nodes\":[],\"p\":\"\\\\{}\"}}", "{".repeat(100));
    assert_eq!(validate_graph(&esc), Ok(()));
}

#[test]
fn twenty_thousand_valid_nodes_validate_in_time() {
    // jsdsp row "20,000 valid nodes in time"
    let nodes: Vec<_> = (0..20_000)
        .map(|i| json!({"type": "mul", "in": [format!("a{i}"), 2], "out": format!("b{i}")}))
        .collect();
    let text = json!({"params": [], "nodes": nodes}).to_string();
    assert!(text.len() < 1 << 20);
    fast("20,000 nodes", || assert_eq!(validate_graph(&text), Ok(())));
}

#[test]
fn twenty_thousand_params_validate_in_time() {
    // decision 351: the duplicate check is a set, not a scan
    let params: Vec<_> = (0..20_000).map(|i| json!({"name": format!("p{i}")})).collect();
    let text = json!({"params": params, "nodes": []}).to_string();
    fast("20,000 params", || assert_eq!(validate_graph(&text), Ok(())));
}

#[test]
fn the_node_table_is_the_engines_set() {
    // decision 351: DspGraph.cpp's `type == "…"` words are the table, so a new engine node is a failing test here
    let cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("engine/k7-runtime/engine/DspGraph.cpp"),
    )
    .expect("DspGraph.cpp");
    let mut found = std::collections::BTreeSet::new();
    let mut rest = cpp.as_str();
    while let Some(i) = rest.find("type == \"") {
        let tail = &rest[i + 9..];
        let end = tail.find('"').expect("a closing quote");
        found.insert(tail[..end].to_owned());
        rest = &tail[end..];
    }
    let table: std::collections::BTreeSet<String> = NODE_TYPES.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(table, found);
    assert_eq!(NODE_TYPES.len(), 32);
}
