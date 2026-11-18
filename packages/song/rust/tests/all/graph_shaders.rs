// graph_shaders.rs — the shader listing of a node: `shadersIn`'s rules, and what the Source's answers do to it (422, 423).
// A reader would get wrong that a stage counts by `existsSync`: a folder is one too, and a dot folder is a shader like any other.

use crate::graph_support;

use graph_support::world;
use serde_json::Value;
use song::graph::{GraphError, Handed, load_web_graph};
use song::{Cache, Entry, MemSource, Source, SourceError, Span, Stat};

fn files<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut all = vec![
        ("lib/k7-core/modules/shaders/base/module.k7x", "type(\"shader\")\n"),
        ("nodes/sh.k7x", "extends(\"@k7-core/shaders/base\")\n"),
        ("graph.k7x", "modules({ sh: [\"OUT\"] })\n"),
    ];
    all.extend_from_slice(extra);
    all
}

/// A Source whose `stat` of the paths ending in `suffix` answers `err`; the rest is the memory's.
struct StatFails<'a> {
    inner: &'a MemSource,
    suffix: &'a str,
    err: SourceError,
}

impl Source for StatFails<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        if path.ends_with(self.suffix) {
            return Err(self.err.clone());
        }
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.inner.read(path, span)
    }
}

fn shaders_of(v: &Value) -> Vec<String> {
    v.pointer("/chains/sh/use/0/shaders")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

// shaders.ts `shadersIn`: no dot filter, and `existsSync` accepts a stage that is a folder.
#[test]
fn a_dot_folder_and_a_stage_that_is_a_folder_are_shaders() {
    let all = files(&[
        ("lib/k7-core/modules/shaders/base/shaders/.dot/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/.dot/fragment.glsl", "f"),
        ("lib/k7-core/modules/shaders/base/shaders/dirstage/vertex.glsl/x", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/dirstage/fragment.glsl", "f"),
        ("lib/k7-core/modules/shaders/base/shaders/soft/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/soft/fragment.glsl", "f"),
        ("lib/k7-core/modules/shaders/base/shaders/half/vertex.glsl", "v"),
    ]);
    let d = graph_support::draw(&all);
    assert_eq!(shaders_of(&d.json), [".dot", "dirstage", "soft"]);
}

fn run(src: &dyn Source, roots: &song::Roots) -> Result<song::graph::Answer, GraphError> {
    load_web_graph(
        src,
        &mut Cache::new(),
        roots,
        song::lanes::rig::Rig::default(),
        &Handed::default(),
    )
}

// 423 and the throw-or-finding table: a bundle without the stage is not an answer of "no shader", it asks for more files.
#[test]
fn a_stage_a_partial_bundle_does_not_hold_is_missing_not_absent() {
    let all = files(&[
        ("lib/k7-core/modules/shaders/base/shaders/soft/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/soft/fragment.glsl", "f"),
    ]);
    let (mem, roots) = world(&all);
    let partial = StatFails {
        inner: &mem,
        suffix: "soft/fragment.glsl",
        err: SourceError::Missing,
    };
    assert!(matches!(
        run(&partial, &roots),
        Err(GraphError::Source(SourceError::Missing))
    ));
}

// 423: a stage the Source refuses is not offered, with one warning for the folder, as a refused listing is.
#[test]
fn a_stage_the_source_refuses_is_not_offered_with_one_warning() {
    let all = files(&[
        ("lib/k7-core/modules/shaders/base/shaders/a/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/a/fragment.glsl", "f"),
        ("lib/k7-core/modules/shaders/base/shaders/b/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/b/fragment.glsl", "f"),
    ]);
    let (mem, roots) = world(&all);
    let linked = StatFails {
        inner: &mem,
        suffix: "/vertex.glsl",
        err: SourceError::Outside,
    };
    let a = run(&linked, &roots).expect("graph");
    let j = serde_json::to_value(&a.graph).expect("json");
    assert!(shaders_of(&j).is_empty());
    let warnings: Vec<_> = a
        .findings
        .iter()
        .filter(|f| f.message.contains("outside the project"))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}
