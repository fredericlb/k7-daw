// graph_choices.rs — what a node's fields may be set to: assets, bases, shaders, the libraries' files, by the folders a node walks (422 to 424).
// A reader would get wrong that a folder is listed ONCE for the whole graph however many nodes share it, and that a refused folder is empty, not a failure.

use crate::graph_support;

use graph_support::{Drawn, draw, draw_with, fast, world};
use serde_json::{Value, json};
use song::graph::{BUDGET, Handed};
use song::{Cache, Entry, Kind, MemSource, Source, SourceError, Span, Stat};
use std::cell::Cell;

fn at<'a>(d: &'a Drawn, ptr: &str) -> Option<&'a Value> {
    d.json.pointer(ptr)
}

fn strings(d: &Drawn, ptr: &str) -> Vec<String> {
    at(d, ptr)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

const AMP: &str = "plugin({ type: \"internal\", name: \"Amp\" })\n";

// moduleAssetChoices.test.ts "lists the module's own assets", moduleAssetsList.test.ts: every format a param takes, nested kept, sorted.
#[test]
fn a_nodes_own_assets_are_listed_nested_sorted_and_only_the_formats_a_param_takes() {
    let d = draw(&[
        ("nodes/x.k7x", AMP),
        ("nodes/assets/z.onnx", "x"),
        ("nodes/assets/a.NAM", "x"),
        ("nodes/assets/sub/b.json", "x"),
        ("nodes/assets/notes.txt", "x"),
        ("nodes/assets/.hidden.nam", "x"),
        ("graph.k7x", "modules({ x: [\"OUT\"] })\n"),
    ]);
    assert_eq!(
        strings(&d, "/chains/x/use/0/assetChoices"),
        ["a.NAM", "sub/b.json", "z.onnx"]
    );
}

// moduleAssetChoices.test.ts "INHERITS", "walks more than one level", "lets a nearer file shadow an inherited one", "cannot loop forever".
#[test]
fn assets_are_inherited_nearest_first_shadowed_and_a_cycle_ends() {
    let lib = |n: &str| format!("lib/k7-core/modules/audio/{n}/module.k7x");
    let files = [
        (lib("top"), AMP.to_owned()),
        (lib("mid"), "extends(\"@k7-core/audio/top\")\n".to_owned()),
        (
            "lib/k7-core/modules/audio/top/assets/shared.nam".to_owned(),
            "x".to_owned(),
        ),
        (
            "lib/k7-core/modules/audio/top/assets/top.nam".to_owned(),
            "x".to_owned(),
        ),
        (
            "lib/k7-core/modules/audio/mid/assets/shared.nam".to_owned(),
            "x".to_owned(),
        ),
        (
            "lib/k7-core/modules/audio/mid/assets/mid.nam".to_owned(),
            "x".to_owned(),
        ),
        ("nodes/n.k7x".to_owned(), "extends(\"@k7-core/audio/mid\")\n".to_owned()),
        ("nodes/assets/own.nam".to_owned(), "x".to_owned()),
        ("graph.k7x".to_owned(), "modules({ n: [\"OUT\"] })\n".to_owned()),
    ];
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = draw(&refs);
    assert_eq!(
        strings(&d, "/chains/n/use/0/assetChoices"),
        ["own.nam", "mid.nam", "shared.nam", "top.nam"]
    );
    // A cycle in `extends` ends at the ancestry's bound and draws.
    let cyc = [
        (
            "lib/k7-core/modules/audio/a/module.k7x",
            "extends(\"@k7-core/audio/b\")\n",
        ),
        (
            "lib/k7-core/modules/audio/b/module.k7x",
            "extends(\"@k7-core/audio/a\")\n",
        ),
        ("lib/k7-core/modules/audio/a/assets/a.nam", "x"),
        ("nodes/n.k7x", "extends(\"@k7-core/audio/a\")\n"),
        ("graph.k7x", "modules({ n: [\"OUT\"] })\n"),
    ];
    let d = fast("cycle", 5, || draw(&cyc));
    assert!(d.json.pointer("/chains/n").is_some());
}

// 407 and the brief's row: 17 levels of extends with an assets folder each reach 16 levels.
#[test]
fn seventeen_levels_of_extends_are_climbed_sixteen() {
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..18 {
        let parent = if i + 1 < 18 {
            format!("extends(\"@k7-core/audio/l{}\")\n", i + 1)
        } else {
            AMP.to_owned()
        };
        files.push((format!("lib/k7-core/modules/audio/l{i}/module.k7x"), parent));
        files.push((
            format!("lib/k7-core/modules/audio/l{i}/assets/f{i}.nam"),
            "x".to_owned(),
        ));
    }
    files.push(("nodes/n.k7x".into(), "extends(\"@k7-core/audio/l0\")\n".into()));
    files.push(("graph.k7x".into(), "modules({ n: [\"OUT\"] })\n".into()));
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = draw(&refs);
    let got = strings(&d, "/chains/n/use/0/assetChoices");
    // The node's own folder holds none, then l0 to l15.
    assert_eq!(got.len(), 16, "{got:?}");
    assert_eq!(got.first().map(String::as_str), Some("f0.nam"));
    assert_eq!(got.last().map(String::as_str), Some("f15.nam"));
}

// basePreset.test.ts: bases without the extension, sorted, a subfolder in the name, .fxp beside .vstpreset, the ancestors' too.
#[test]
fn bases_are_named_without_their_extension_and_inherited() {
    let d = draw(&[
        (
            "lib/k7-core/modules/instruments/surge/module.k7x",
            "plugin({ type: \"internal\", name: \"Synth\" })\n",
        ),
        ("lib/k7-core/modules/instruments/surge/base/warm.vstpreset", "x"),
        ("lib/k7-core/modules/instruments/surge/base/Pads/Bell Pad.fxp", "x"),
        ("lib/k7-core/modules/instruments/surge/base/readme.txt", "x"),
        (
            "nodes/s.k7x",
            "extends(\"@k7-core/instruments/surge\")\nbase(\"Pads/Bell Pad\")\n",
        ),
        ("nodes/bare.k7x", AMP),
        ("graph.k7x", "modules({ s: [\"OUT\"], bare: [\"OUT\"] })\n"),
    ]);
    assert_eq!(strings(&d, "/chains/s/use/0/bases"), ["Pads/Bell Pad", "warm"]);
    assert_eq!(at(&d, "/chains/s/use/0/base"), Some(&json!("Pads/Bell Pad")));
    assert_eq!(at(&d, "/chains/bare/use/0/bases"), None);
}

// shaders.test.ts: the module's own shaders sorted, half a shader is not one, the ancestry, only for the shaders category.
#[test]
fn shaders_are_discovered_whole_and_only_for_a_shader_module() {
    let d = draw(&[
        ("lib/k7-core/modules/shaders/base/module.k7x", "type(\"shader\")\n"),
        ("lib/k7-core/modules/shaders/base/shaders/blur/vertex.glsl", "v"),
        ("lib/k7-core/modules/shaders/base/shaders/blur/fragment.glsl", "f"),
        ("lib/k7-core/modules/shaders/base/shaders/half/vertex.glsl", "v"),
        ("nodes/sh.k7x", "extends(\"@k7-core/shaders/base\")\nshader(\"blur\")\n"),
        ("nodes/amp.k7x", AMP),
        ("nodes/amp/shaders/blur/vertex.glsl", "v"),
        ("graph.k7x", "modules({ sh: [\"OUT\"], amp: [\"OUT\"] })\n"),
    ]);
    assert_eq!(strings(&d, "/chains/sh/use/0/shaders"), ["blur"]);
    assert_eq!(at(&d, "/chains/amp/use/0/shaders"), None);
}

// libraryFiles.test.ts: a family's sound files as @<library>/<path>, the project's first; by bind, by `library`, by a rack's taps.
#[test]
fn library_files_come_by_bind_by_library_and_for_a_rack_with_the_projects_first() {
    let panel = |bind: &str, library: &str| {
        let kind = if library.is_empty() { "filepicker" } else { "dropdown" };
        format!(
            "{AMP}ui({{ cols: 1, rows: 1, widgets: {{ w: {{ type: \"{kind}\", col: 0, row: 0, {bind}{library} }} }} }})\n"
        )
    };
    let by_bind = panel("bind: \"ir\"", "");
    let by_model = panel("bind: \"model\"", "");
    let by_library = panel("bind: \"slot\", ", "library: \"wavetables\"");
    let rack = "type(\"rack\")\nplugin({ type: \"internal\", name: \"Sampler\" })\n";
    let d = draw(&[
        ("lib/k7-core/irs/hall.wav", "x"),
        ("lib/k7-core/irs/notes.txt", "x"),
        ("lib/k7-core/models/amp.nam", "x"),
        ("lib/k7-core/samples/kick.wav", "x"),
        ("lib/k7-core/samples/drums/snare.flac", "x"),
        ("lib/k7-core/wavetables/saw.wav", "x"),
        ("samples/own.wav", "x"),
        ("wavetables/mine.wav", "x"),
        ("nodes/ir.k7x", by_bind.as_str()),
        ("nodes/mod.k7x", by_model.as_str()),
        ("nodes/wt.k7x", by_library.as_str()),
        ("nodes/rk.k7x", rack),
        (
            "graph.k7x",
            "modules({ ir: [\"OUT\"], mod: [\"OUT\"], wt: [\"OUT\"], rk: [\"OUT\"] })\n",
        ),
    ]);
    // An impulse response is the libraries' alone; a model is the libraries' models; wavetables take the project's too.
    assert_eq!(strings(&d, "/chains/ir/use/0/assetChoices"), ["@k7-core/hall.wav"]);
    assert_eq!(strings(&d, "/chains/mod/use/0/assetChoices"), ["@k7-core/amp.nam"]);
    assert_eq!(
        strings(&d, "/chains/wt/use/0/assetChoices"),
        ["@project/mine.wav", "@k7-core/saw.wav"]
    );
    assert_eq!(
        strings(&d, "/chains/rk/use/0/assetChoices"),
        ["@project/own.wav", "@k7-core/drums/snare.flac", "@k7-core/kick.wav"]
    );
}

/// A Source that counts the listings it is asked, and refuses one folder as `Outside`.
struct Counting<'a> {
    inner: &'a MemSource,
    lists: Cell<usize>,
    refuse: &'a str,
    asked: Cell<usize>,
}

impl Source for Counting<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.lists.set(self.lists.get() + 1);
        if dir.ends_with(self.refuse) {
            self.asked.set(self.asked.get() + 1);
            return Err(SourceError::Outside);
        }
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.inner.read(path, span)
    }
}

// 423 and 424: a folder shared by three nodes is listed once; one the Source refuses is empty, with ONE warning, and no failure.
#[test]
fn a_shared_folder_is_listed_once_and_a_refused_one_is_empty_with_one_warning() {
    let files = [
        ("lib/k7-core/modules/audio/amp/module.k7x", AMP),
        ("lib/k7-core/modules/audio/amp/assets/a.nam", "x"),
        ("nodes/n1.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        ("nodes/n2.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        ("nodes/n3.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        (
            "graph.k7x",
            "modules({ n1: [\"OUT\"], n2: [\"OUT\"], n3: [\"OUT\"] })\n",
        ),
    ];
    let (mem, roots) = world(&files);
    let counting = Counting {
        inner: &mem,
        lists: Cell::new(0),
        refuse: "amp/assets",
        asked: Cell::new(0),
    };
    let a = song::graph::load_web_graph(
        &counting,
        &mut Cache::new(),
        &roots,
        song::lanes::rig::Rig::default(),
        &Handed::default(),
    )
    .expect("graph");
    assert_eq!(
        counting.asked.get(),
        1,
        "the refused folder is asked once for three nodes"
    );
    let j = serde_json::to_value(&a.graph).expect("json");
    assert!(j.pointer("/chains/n1/use/0/assetChoices").is_none());
    let warnings: Vec<_> = a
        .findings
        .iter()
        .filter(|f| f.message.contains("outside the project"))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    // The same project unrefused lists the folder once as well.
    let plain = Counting {
        inner: &mem,
        lists: Cell::new(0),
        refuse: "no-such-suffix",
        asked: Cell::new(0),
    };
    song::graph::load_web_graph(
        &plain,
        &mut Cache::new(),
        &roots,
        song::lanes::rig::Rig::default(),
        &Handed::default(),
    )
    .expect("graph");
    let assets_lists = plain.lists.get();
    assert!(assets_lists < 200, "{assets_lists}");
}

/// A Source whose one folder holds a generated listing of `n` files, the rest being the memory's.
struct Wide<'a> {
    inner: &'a MemSource,
    dir: &'a str,
    names: Vec<String>,
}

impl Source for Wide<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        if dir == self.dir {
            return Ok(self
                .names
                .iter()
                .map(|n| Entry {
                    name: n.clone(),
                    kind: Kind::File,
                })
                .collect());
        }
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.inner.read(path, span)
    }
}

// 419 and the brief's row: 1,000,000 entries in one assets/ are a walk of 1,000,000 steps, which the budget stops.
#[test]
fn a_million_assets_are_stopped_by_the_budget() {
    let files = [("nodes/x.k7x", AMP), ("graph.k7x", "modules({ x: [\"OUT\"] })\n")];
    let (mem, roots) = world(&files);
    let wide = Wide {
        inner: &mem,
        dir: "/p/ws/proj/nodes/assets",
        names: (0..1_000_000).map(|n| format!("a{n}.nam")).collect(),
    };
    let a = fast("a million assets", 5, || {
        song::graph::load_web_graph(
            &wide,
            &mut Cache::new(),
            &roots,
            song::lanes::rig::Rig::default(),
            &Handed::default(),
        )
        .expect("graph")
    });
    assert!(a.capped && a.spent > BUDGET);
    let j = serde_json::to_value(&a.graph).expect("json");
    assert!(j.pointer("/chains/x/use/0/assetChoices").is_none());
    assert_eq!(
        j["chains"].as_object().map(serde_json::Map::len),
        Some(1),
        "the topology is whole"
    );
}

// The brief's row: a shaders/ of 50,000 folders costs two stats each and offers every whole one.
#[test]
fn fifty_thousand_shader_folders_are_two_stats_each() {
    let mut owned: Vec<(String, String)> = vec![
        (
            "lib/k7-core/modules/shaders/big/module.k7x".into(),
            "type(\"shader\")\n".into(),
        ),
        ("nodes/sh.k7x".into(), "extends(\"@k7-core/shaders/big\")\n".into()),
        ("graph.k7x".into(), "modules({ sh: [\"OUT\"] })\n".into()),
    ];
    for n in 0..50_000 {
        owned.push((
            format!("lib/k7-core/modules/shaders/big/shaders/s{n}/vertex.glsl"),
            "v".into(),
        ));
        owned.push((
            format!("lib/k7-core/modules/shaders/big/shaders/s{n}/fragment.glsl"),
            "f".into(),
        ));
    }
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("shaders", 5, || draw(&files));
    assert_eq!(strings(&d, "/chains/sh/use/0/shaders").len(), 50_000);
}

// The brief's row: a library of 30,000 samples and 500 rack steps over it is one walk, charged per copy, then absent.
#[test]
fn a_big_library_over_many_rack_steps_is_walked_once_and_charged_per_copy() {
    let mut owned: Vec<(String, String)> = vec![(
        "lib/k7-core/modules/instruments/rk/module.k7x".into(),
        "type(\"rack\")\nplugin({ type: \"internal\", name: \"Sampler\" })\n".into(),
    )];
    let mut graph = Vec::new();
    for n in 0..500 {
        owned.push((
            format!("nodes/r{n}.k7x"),
            "extends(\"@k7-core/instruments/rk\")\n".into(),
        ));
        graph.push(format!("r{n}: [\"OUT\"]"));
    }
    for n in 0..30_000 {
        owned.push((format!("lib/k7-core/samples/s{n:05}.wav"), "x".into()));
    }
    owned.push(("graph.k7x".into(), format!("modules({{ {} }})\n", graph.join(", "))));
    let files: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = fast("library", 5, || draw_with(&files, &json!({}), &Handed::default()));
    assert!(d.capped);
    let chains = d.json["chains"].as_object().expect("chains");
    let full = chains
        .values()
        .filter(|c| c["use"][0].get("assetChoices").is_some())
        .count();
    assert!(full > 0 && full < 500, "{full} steps paid for");
}

// 444: a node named with a separator or a drive colon is stepped, and what it lists is its folder's, never a path its name makes.
#[test]
fn a_node_name_is_not_a_folder_in_a_walk() {
    let files = [
        ("nodes/..\\x.k7x", AMP),
        ("nodes/c:x.k7x", AMP),
        ("nodes/assets/ok.nam", "x"),
        ("x/assets/leak.nam", "x"),
        ("graph.k7x", "modules({ \"..\\\\x\": [\"OUT\"], \"c:x\": [\"OUT\"] })\n"),
    ];
    let d = draw(&files);
    for name in ["..\\x", "c:x"] {
        let ptr = format!("/chains/{name}/use/0");
        assert!(at(&d, &ptr).is_some(), "{name} is not drawn: {}", d.json);
        assert_eq!(strings(&d, &format!("{ptr}/assetChoices")), ["ok.nam"], "{name}");
    }
}
