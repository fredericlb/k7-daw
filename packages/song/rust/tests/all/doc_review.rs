// doc_review.rs — what the review of package B found: the orders JavaScript gives, the stops in the diagnostics, a repeated owner, the budgets.
// A reader would get wrong that each case is the TypeScript's answer, held by a `ts: true` case of doc_negative.json too (decisions 381, 391 to 395).

use crate::doc_support;

use doc_support::{AMP, compiled, doc_of, lane, stopped};
use song::doc::{BUDGET, Compiled};
use song::files::Severity;
use std::time::{Duration, Instant};

const P: &str = "project({ name: \"n\" })\n";

fn ids(c: &song::doc::ProjectDoc) -> Vec<&str> {
    c.tracks.iter().map(|t| t.id.as_str()).collect()
}

fn chain_of(id: &str, track: &str) -> String {
    format!("start(true)\nsteps([\n  id(\"{id}\").track(\"{track}\"),\n  to([chain(\"OUT\")]),\n])\n")
}

// 381, 392: a chain project keeps the chain files' order, where a graph project is sorted as `[base, name]` pairs.
#[test]
fn a_chain_project_keeps_the_order_of_its_chain_files() {
    let (a, b) = (chain_of("a", "zeta"), chain_of("b", "alpha"));
    let d = doc_of(&[
        ("project.k7x", P),
        ("chains/a.k7x", &a),
        ("chains/b.k7x", &b),
        ("tracks/zeta.k7x", &lane("")),
        ("tracks/alpha.k7x", &lane("")),
    ]);
    assert_eq!(ids(&d), ["zeta", "alpha"]);
}

// 381: a graph project's nodes sort as the pairs `[base, name]`, so `fx-a` comes before `fx-a-room`.
#[test]
fn a_graph_projects_nodes_sort_as_pairs() {
    let d = doc_of(&[
        ("project.k7x", P),
        (
            "graph.k7x",
            "modules({ \"fx-a-room\": [\"MIX\"], \"fx-a\": [\"MIX\"] })\n",
        ),
        ("nodes/fx-a-room.k7x", &lane("")),
        ("nodes/fx-a.k7x", &lane("")),
    ]);
    assert_eq!(ids(&d), ["fx-a", "fx-a-room"]);
}

// 391: timelines `a` and `a-b` are listed as `[base, name]` pairs sort them (`-` is below `.`, so bytewise puts `a-b` first).
#[test]
fn timelines_follow_the_order_of_pairs() {
    let t = "lanes([lane(\"l\").events([ref(\"k\").start(\"1:1\")])])\n";
    let d = doc_of(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"gtr\": [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", &lane("")),
        ("timelines/a-b.k7x", t),
        ("timelines/a.k7x", t),
        ("clips/k.k7x", "source(\"clips/k.wav\")\n"),
        ("clips/k.wav", "w"),
    ]);
    let got: Vec<&str> = d.timelines.iter().flatten().map(|t| t.id.as_str()).collect();
    assert_eq!(got, ["a", "a-b"]);
}

// 392: JavaScript lists integer-like names first and ascending: `9` before `10`, in nodes and in chain files.
#[test]
fn integer_like_lane_names_come_first_and_ascending() {
    let d = doc_of(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"10\": [\"MIX\"], \"9\": [\"MIX\"] })\n"),
        ("nodes/10.k7x", &lane("")),
        ("nodes/9.k7x", &lane("")),
    ]);
    assert_eq!(ids(&d), ["9", "10"]);
    let (x, y) = (chain_of("x", "10"), chain_of("y", "9"));
    let d = doc_of(&[
        ("project.k7x", P),
        ("chains/a.k7x", &x),
        ("chains/b.k7x", &y),
        ("tracks/10.k7x", &lane("")),
        ("tracks/9.k7x", &lane("")),
    ]);
    assert_eq!(ids(&d), ["9", "10"]);
}

// 392: the chains are an object too, so buses follow the first chain that sends to them in ascending integer order.
#[test]
fn chains_named_by_integers_are_listed_ascending_before_the_buses_are_found() {
    let d = doc_of(&[
        ("project.k7x", P),
        (
            "graph.k7x",
            "modules({ \"10\": [\"bx\"], \"12\": [\"bx\"], \"9\": [\"by\"], \"11\": [\"by\"], \"bx\": [\"MIX\"], \"by\": [\"MIX\"] })\n",
        ),
        ("nodes/9.k7x", &lane("")),
        ("nodes/10.k7x", &lane("")),
        ("nodes/11.k7x", &lane("")),
        ("nodes/12.k7x", &lane("")),
        ("nodes/bx.k7x", AMP),
        ("nodes/by.k7x", AMP),
    ]);
    assert_eq!(ids(&d), ["9", "10", "11", "12", "by", "bx"]);
}

// 392: a rack's sounds are listed in the order of its symbols as an object keeps them: `4` before `36`.
#[test]
fn rack_symbols_that_are_integers_come_first_and_ascending() {
    let rack = "type(\"rack\")\nrack({ rack_type: \"samples\", symbols: { \"36\": { note: 36, clip: \"k.wav\" }, \"4\": { note: 4, clip: \"k.wav\" } } })\n";
    let d = doc_of(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"gtr\": [\"r\"], \"r\": [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/r.k7x", rack),
        ("k.wav", "x"),
    ]);
    let symbols: Vec<&str> = d.tracks[0].plugins[0]
        .rack
        .iter()
        .flatten()
        .map(|s| s.symbol.as_str())
        .collect();
    assert_eq!(symbols, ["4", "36"]);
}

// 318: a Faust file under the workspace's modules is inside the project roots, so the slot keeps it.
#[test]
fn a_faust_file_in_the_workspace_modules_is_kept() {
    let d = doc_of(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"gtr\": [\"f\"], \"f\": [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", &lane("")),
        (
            "nodes/f.k7x",
            "plugin({ type: \"faust\", name: \"f\" })\nparams({ Table: \"../_modules/t.wav\", Away: \"../../../etc/x.wav\" })\n",
        ),
        ("nodes/dsp.dsp", "process = _;\n"),
        ("ws/_modules/t.wav", "w"),
    ]);
    let files = d.tracks[0].plugins[0].faust_files.as_ref().expect("faust files");
    assert_eq!(files.get("Table").map(String::as_str), Some("/p/ws/_modules/t.wav"));
    // A path past every root is the Source's `Outside`, and the slot is left out.
    assert!(!files.contains_key("Away"), "{files:?}");
}

// 380: a failure's stops are in the compile's diagnostics too, the references and wirings among them.
#[test]
fn a_failure_keeps_its_stops_in_the_diagnostics() {
    let c = compiled(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"gtr\": [\"amp\"], \"amp\": [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/amp.k7x", "extends(\"@nope/audio/x\")\n"),
    ]);
    let f = c.engine.as_ref().expect_err("a reference to nothing stops the compile");
    assert!(!f.diagnostics.is_empty());
    for d in &f.diagnostics {
        assert!(
            c.diagnostics
                .iter()
                .any(|a| a.file == d.file && a.message == d.message && a.severity == d.severity),
            "{d:?} is not among {:?}",
            c.diagnostics
        );
    }
}

// 394: a chain that is an input and fed by a tap is two tracks, and each has the chain's plugins.
#[test]
fn a_chain_that_is_an_input_and_tap_fed_gives_both_tracks_its_plugins() {
    let d = doc_of(&[
        ("project.k7x", P),
        (
            "graph.k7x",
            "modules({ \"gtr\": [\"kit\"], \"kit\": [\"MIX\"], \"rec\": [\"amp\"], \"amp\": [\"MIX\"] })\ntap_feeds([{ node: \"kit\", tap: \"Snare\", to: \"rec\" }])\n",
        ),
        ("nodes/gtr.k7x", &lane("")),
        (
            "nodes/kit.k7x",
            "plugin({ type: \"internal\", name: \"Sampler\" })\nkit({ Snare: 38 })\ntap_outs({ Snare: \"rec\" })\n",
        ),
        ("nodes/rec.k7x", "io(\"in\")\nchannel(\"mic\")\n"),
        ("nodes/amp.k7x", AMP),
    ]);
    assert_eq!(ids(&d), ["gtr", "rec", "rec"]);
    let counts: Vec<usize> = d.tracks.iter().map(|t| t.plugins.len()).collect();
    assert_eq!(counts, [1, 1, 1]);
    assert_eq!(d.tracks[1].plugins[0].id, d.tracks[2].plugins[0].id);
}

fn hang_bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn faust_chain(k: usize, tune: usize, curve: bool) -> Compiled {
    const STEPS: usize = 62;
    let mut files = vec![
        ("project.k7x".to_owned(), P.to_owned()),
        ("nodes/gtr.k7x".to_owned(), lane("")),
        ("nodes/dsp.dsp".to_owned(), "process = _;\n".repeat(k)),
        (
            "curves/c.k7x".to_owned(),
            "type(\"custom\")\npoints([time(\"1:1:1\").value(0), time(\"2:1:1\").value(1)])\n".to_owned(),
        ),
    ];
    let mut pairs = Vec::new();
    for i in 0..STEPS {
        let autos = if curve && i == 0 {
            "automations([{ param: \"p\", curve: \"c\" }])\n"
        } else {
            ""
        };
        files.push((
            format!("nodes/f{i}.k7x"),
            format!("plugin({{ type: \"faust\", name: \"f\" }})\n{autos}"),
        ));
        let from = if i == 0 {
            "gtr".to_owned()
        } else {
            format!("f{}", i - 1)
        };
        pairs.push(format!("\"{from}\": [\"f{i}\"]"));
    }
    let params: Vec<String> = (0..tune).map(|i| format!("q{i}: 1")).collect();
    files.push((
        "nodes/tune.k7x".to_owned(),
        format!(
            "plugin({{ type: \"internal\", name: \"Chorus\" }})\nparams({{ {} }})\n",
            params.join(", ")
        ),
    ));
    pairs.push(format!("\"f{}\": [\"tune\"]", STEPS - 1));
    pairs.push("\"tune\": [\"MIX\"]".to_owned());
    files.push(("graph.k7x".to_owned(), format!("modules({{ {} }})\n", pairs.join(", "))));
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    compiled(&refs)
}

// 385: a doc budget spent after the plugins, by the bake, is the one ERROR and no document.
#[test]
fn a_budget_spent_by_the_bake_is_an_error_and_no_document() {
    let t = Instant::now();
    // The smallest Faust source whose plugins alone pass the budget.
    let (mut lo, mut hi) = (1, 80_000);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if faust_chain(mid, 0, false).engine.is_err() {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let mut at_the_bake = 0;
    for tune in 0..70 {
        let c = faust_chain(lo - 1, tune, true);
        if c.spent.doc > BUDGET {
            let f = c.engine.as_ref().expect_err("a spent budget gives no document");
            at_the_bake += usize::from(f.diagnostics.iter().any(|d| d.file == "project.k7x"));
        }
    }
    assert!(at_the_bake > 0, "no run spent the budget after its plugins");
    assert!(t.elapsed() < hang_bound() * 6, "{:?}", t.elapsed());
}

fn name_of(len: usize) -> String {
    format!("plugin({{ type: \"internal\", name: \"{}\" }})\n", "N".repeat(len))
}

// 387: a plugin's own name is weighed by its length, in the document's budget.
#[test]
fn a_plugins_name_is_charged_by_its_length() {
    let c = compiled(&[
        ("project.k7x", P),
        ("graph.k7x", "modules({ \"gtr\": [\"amp\"], \"amp\": [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", &lane("")),
        ("nodes/amp.k7x", &name_of(1 << 20)),
    ]);
    assert!(c.spent.doc >= (1 << 20) / 16, "{:?}", c.spent);
}

// 387, 393: a library's plugin name of 1 MiB extended by 400 nodes is stopped by the resolver, not copied 400 times.
#[test]
fn a_big_plugin_name_extended_by_many_nodes_is_stopped_by_the_resolver() {
    let mut owned = vec![
        ("project.k7x".to_owned(), P.to_owned()),
        ("nodes/gtr.k7x".to_owned(), lane("")),
        (
            "lib/k7-core/library.k7x".to_owned(),
            "name(\"k7-core\")\nversion(\"1.0.0\")\n".to_owned(),
        ),
        ("lib/k7-core/modules/audio/big/module.k7x".to_owned(), name_of(1 << 20)),
    ];
    let mut pairs = vec!["\"gtr\": [\"n0\"]".to_owned()];
    for i in 0..400 {
        owned.push((
            format!("nodes/n{i}.k7x"),
            "extends(\"@k7-core/audio/big\")\n".to_owned(),
        ));
        let to = if i == 399 {
            "MIX".to_owned()
        } else {
            format!("n{}", i + 1)
        };
        pairs.push(format!("\"n{i}\": [\"{to}\"]"));
    }
    owned.push(("graph.k7x".to_owned(), format!("modules({{ {} }})\n", pairs.join(", "))));
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let t = Instant::now();
    let c = compiled(&refs);
    assert!(t.elapsed() < hang_bound(), "{:?}", t.elapsed());
    assert!(c.engine.is_err(), "400 copies of a 1 MiB name were built");
    assert!(
        c.diagnostics
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.starts_with("resolving holds more than")),
        "{:?}",
        stopped(&c)
    );
}
