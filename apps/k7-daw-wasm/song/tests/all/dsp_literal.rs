// dsp_literal.rs — every module's `dsp.k7x` decodes byte for byte to the graph the engine reads, frozen in dsp_vectors.json.
// A reader would get wrong where the references come from: the file is a golden with no generator, so a module's DSP change edits it by hand.

use song::dsp_literal::decode;
use song::jsdsp::validate_graph;
use std::path::PathBuf;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").join(rel)
}

fn vectors() -> Vec<(String, String)> {
    let text = include_str!("dsp_vectors.json");
    let rows: Vec<serde_json::Value> = serde_json::from_str(text).expect("the vectors");
    rows.iter()
        .map(|r| {
            (
                r["module"].as_str().unwrap().to_string(),
                r["graph"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn every_dsp_k7x_decodes_to_the_graph_its_jsdsp_ts_builds() {
    let rows = vectors();
    assert_eq!(
        rows.len(),
        23,
        "the 21 k7-core modules, the demos' and the e2e fixtures' crossfade"
    );
    for (module, graph) in rows {
        let path = format!("{module}/dsp.k7x");
        let text = std::fs::read_to_string(repo(&path)).unwrap_or_else(|_| panic!("{path} is missing"));
        let got = decode(&path, &text).unwrap_or_else(|f| panic!("{f:?}"));
        validate_graph(&got).expect("a graph the engine builds");
        assert_eq!(got, graph, "{module}");
    }
}

#[test]
fn a_param_spec_may_be_a_chain_and_a_file_may_hold_nothing() {
    let chain = decode("m/dsp.k7x", "params({ Mix: min(0).max(1).default(0.5) })").expect("decodes");
    assert!(chain.contains("\"default\": 0.5"));
    assert_eq!(
        decode("m/dsp.k7x", "").expect("decodes"),
        "{\n  \"params\": [],\n  \"nodes\": []\n}\n"
    );
}
