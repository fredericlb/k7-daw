// graph_support/mod.rs — what the 2k tests share: a project laid in memory, loaded to a graph, asked for its sentences and keys.
// A reader would get wrong that a graph borrows its `Inputs`, so the helpers hand a closure the graph instead of returning it.

#![allow(dead_code)]

mod cases;
mod gate;
#[allow(unused_imports)]
pub use cases::*;
#[allow(unused_imports)]
pub use gate::*;

use crate::support::{mem, mem_world_roots};
use serde_json::{Value, json};
use song::graph::{GraphDoc, GraphError, Refusal, load_graph};
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::{Cache, MemSource, Roots};

/// The files of a project (paths below the project), plus any library or `@meta` file written as `lib/...` or `meta/...`, and `scan.json` as the engine's scan file.
pub fn laid(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(rel, text)| {
            let path = if rel.starts_with("lib/") || rel.starts_with("meta/") {
                (*rel).to_owned()
            } else if *rel == "scan.json" {
                "exts.json".to_owned()
            } else {
                format!("ws/proj/{rel}")
            };
            (path, (*text).to_owned())
        })
        .collect()
}

/// A memory source over the laid files, and the roots that read it.
pub fn world(files: &[(&str, &str)]) -> (MemSource, Roots) {
    let owned = laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut roots = mem_world_roots();
    if files.iter().any(|(rel, _)| *rel == "scan.json") {
        roots.scan = Some("/p/exts.json".into());
    }
    (mem(&refs), roots)
}

pub fn inputs_with(src: &MemSource, roots: &Roots, rig: &Value) -> Inputs {
    load_inputs(src, &mut Cache::new(), roots, Rig::from_json(rig)).expect("inputs")
}

/// Loads the graph of the laid files and hands it to `f`.
pub fn with_graph<R>(files: &[(&str, &str)], f: impl FnOnce(&GraphDoc<'_>) -> R) -> R {
    with_graph_rig(files, &json!({}), f)
}

pub fn with_graph_rig<R>(files: &[(&str, &str)], rig: &Value, f: impl FnOnce(&GraphDoc<'_>) -> R) -> R {
    let (src, roots) = world(files);
    let inputs = inputs_with(&src, &roots, rig);
    let g = load_graph(&src, &mut Cache::new(), &roots, &inputs).unwrap_or_else(|e| panic!("graph refused: {e:?}"));
    f(&g)
}

/// What a graph is refused with, or a panic when it loads.
pub fn refused(files: &[(&str, &str)]) -> Refusal {
    let (src, roots) = world(files);
    let inputs = inputs_with(&src, &roots, &json!({}));
    match load_graph(&src, &mut Cache::new(), &roots, &inputs) {
        Err(GraphError::Refused(r)) => r,
        Err(e) => panic!("not a refusal: {e:?}"),
        Ok(_) => panic!("the graph loaded"),
    }
}

/// `extends("@meta/track")` with the lines given after it.
pub fn lane(extra: &str) -> String {
    format!("extends(\"@meta/track\")\n{extra}")
}

pub const OUT_AMP: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\n";

/// The keys of the graph's issues, sorted: `code|name=value|…`.
pub fn issue_keys(g: &GraphDoc<'_>) -> Vec<String> {
    let mut k: Vec<String> = g.issues.iter().map(song::graph::GraphIssue::key).collect();
    k.sort();
    k
}

pub fn codes(g: &GraphDoc<'_>) -> Vec<&'static str> {
    g.issues.iter().map(|i| i.code.name()).collect()
}

/// Runs `f`, asserts it took under three times `secs` (twelve on musl): a hang bound a loaded Mac does not trip.
pub fn fast<T>(what: &str, secs: u64, f: impl FnOnce() -> T) -> T {
    let t = std::time::Instant::now();
    let out = f();
    let bound = std::time::Duration::from_secs(if cfg!(target_env = "musl") { secs * 12 } else { secs * 3 });
    assert!(t.elapsed() < bound, "{what} took {:?}", t.elapsed());
    out
}

/// Loads the graph, lets `mutate` change it (a state no file can make), and gives the keys `validate_issues` answers.
pub fn revalidated(files: &[(&str, &str)], mutate: impl FnOnce(&mut GraphDoc<'_>)) -> Vec<String> {
    let (src, roots) = world(files);
    let inputs = inputs_with(&src, &roots, &json!({}));
    let mut g = load_graph(&src, &mut Cache::new(), &roots, &inputs).expect("graph");
    mutate(&mut g);
    let mut k: Vec<String> = song::graph::validate_issues(&g, &inputs.vocabulary)
        .iter()
        .map(song::graph::GraphIssue::key)
        .collect();
    k.sort();
    k
}

/// The sorted issue keys of the laid files' graph.
pub fn keys(files: &[(&str, &str)]) -> Vec<String> {
    with_graph(files, issue_keys)
}

/// The sentences of the laid files' graph, in order.
pub fn sentences(files: &[(&str, &str)]) -> Vec<String> {
    with_graph(files, |g| g.warnings.clone())
}

/// The JSON of the graph the laid files draw, with the handed values and the rig, and the answer's findings and flags.
pub struct Drawn {
    pub json: Value,
    pub findings: Vec<song::Finding>,
    pub capped: bool,
    pub spent: usize,
}

pub fn draw_with(files: &[(&str, &str)], rig: &Value, handed: &song::graph::Handed) -> Drawn {
    let (src, roots) = world(files);
    let a = song::graph::load_web_graph(&src, &mut Cache::new(), &roots, Rig::from_json(rig), handed)
        .unwrap_or_else(|e| panic!("graph refused: {e:?}"));
    Drawn {
        json: serde_json::to_value(&a.graph).expect("json"),
        findings: a.findings,
        capped: a.capped,
        spent: a.spent,
    }
}

pub fn draw(files: &[(&str, &str)]) -> Drawn {
    draw_with(files, &json!({}), &song::graph::Handed::default())
}

/// A `null` or a float in an integer field anywhere in `v`: the output rules (438).
pub fn no_null(v: &Value, at: &str, bad: &mut Vec<String>) {
    match v {
        Value::Null => bad.push(format!("{at}: null")),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, x)| no_null(x, &format!("{at}/{i}"), bad)),
        Value::Object(o) => {
            for (k, x) in o {
                no_null(x, &format!("{at}/{k}"), bad);
                let integer = matches!(k.as_str(), "note" | "channel" | "knobs" | "ch" | "cc");
                // An integer field is a number, or an array of them (`channel`).
                let numbers: Vec<&Value> = match x {
                    Value::Array(a) => a.iter().collect(),
                    other => vec![other],
                };
                for n in numbers {
                    if integer
                        && let Value::Number(n) = n
                        && !(n.is_u64() || n.is_i64())
                    {
                        bad.push(format!("{at}/{k}: not an integer"));
                    }
                }
            }
        }
        _ => {}
    }
}
