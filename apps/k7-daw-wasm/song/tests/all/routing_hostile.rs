// routing_hostile.rs — the routing's hostile inputs, each refused fast with the answer asserted, never just "it did not crash".
// A reader would get wrong that a row counts the OUTPUT: a 30-byte file can name a million segments (decision 105).

use indexmap::IndexMap;
use song::routing::{Budget, Segments, decode_chain, decode_graph, decode_layout, derive_chains, to_segments};
use std::time::{Duration, Instant};

fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(
        t.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{what} took {:?}",
        t.elapsed()
    );
    out
}

fn nest(n: usize) -> String {
    format!("{}1{}", "[".repeat(n), "]".repeat(n))
}

#[test]
fn graph_files_that_are_hostile() {
    // Decisions 90, 105, 110: each is refused, and the long ones in time.
    let deep = format!("modules({{ a: {} }})\n", nest(300));
    let cases = [
        deep,
        "runs([[\"a\"]])\n".to_owned(),
        "runs(\"a\")\n".to_owned(),
        "runs([[\"a\", 1]])\n".to_owned(),
        "modules(3)\nruns([[\"a\", \"b\"]])\n".to_owned(),
        "modules({ a: [\"#x\"] })\n".to_owned(),
        "modules({ a: [\"x#\"] })\n".to_owned(),
        "modules({ \"\": [\"x\"] })\n".to_owned(),
        "positions({ a: { x: 1, y: 2, extra: 3 } })\n".to_owned(),
        "positions({ a: { x: \"1\", y: 2 } })\n".to_owned(),
    ];
    for c in &cases {
        fast("a refused graph", || decode_graph("graph.k7x", c).expect_err(c));
    }
}

#[test]
fn a_runs_past_the_budget_is_one_error_on_graph_k7x() {
    // Decision 105: 1,500,000 names in 750,000 runs of two; the file is named, nothing is built.
    let names: String = (0..750_000).map(|i| format!("[\"a{i}\",\"b{i}\"],")).collect();
    let text = format!("runs([{names}])\n");
    let f = fast("a runs past the budget", || {
        decode_graph("graph.k7x", &text).expect_err("capped")
    });
    assert_eq!(f.file, "graph.k7x");
}

#[test]
fn a_graph_of_300_000_keys_each_feeding_the_next_decodes() {
    let body: String = (0..300_000).map(|i| format!("n{i}:[\"n{}\"],", i + 1)).collect();
    let text = format!("modules({{{body}}})\n");
    let s = fast("300,000 keys", || decode_graph("graph.k7x", &text).expect("a graph"));
    assert_eq!(s.modules.expect("modules").len(), 300_000);
}

fn chain(steps: &str) -> String {
    format!("start(true)\nsteps([{steps}])\n")
}

#[test]
fn chain_files_that_are_hostile() {
    // Decisions 91 to 93: the table of the work order, each refused.
    let n = "id(\"a\").extends(\"x\")";
    let deep = format!("steps([{}])\n", nest(300));
    let cases = [
        deep,
        chain(&format!("{n}, to([chain(\"OUT\").level(-1)])")),
        chain(&format!("{n}, to([chain(\"OUT\").level(1e400)])")),
        "start(false)\n".to_owned(),
        chain(&format!("{n}, {{to: []}}")),
        chain(&format!("{n}, to([chain(\"b\"), chain(\"c\")])")),
        chain(&format!("{n}, [{n}]")),
        chain(&format!("{n}, [[{n}, {n}], {n}]")),
        chain("\"a\""),
        chain(&format!("{n}, {{to: [{{chain: \"OUT\"}}], id: \"z\"}}")),
        chain(&format!("{n}, {{sub: \"s\", extends: \"x\"}}")),
        chain("{sub: \"s\"}"),
        chain("id(\"\").extends(\"x\")"),
        chain("id(\"#a\").extends(\"x\")"),
        chain("id(\"a#\").extends(\"x\")"),
        chain("id(\"l\").track(\"../../x\")"),
        chain("id(\"l\").track(\"a/b\")"),
        chain("id(\"l\").track(\"a\\u0000b\")"),
        chain("id(\"a\").extends(\"x\"), to([chain(\"t\").port(\"p#\")])"),
        "steps([id(\"a\").extends(\"x\"), {sub: \"s\"}, {sub: \"s\"}])\nsubchains([{id: \"s\", steps: [id(\"d\").extends(\"w\")]}])\n".to_owned(),
        "steps([id(\"a\").extends(\"x\")])\nsubchains([{id: \"OUT\", steps: [id(\"d\").extends(\"w\")]}])\n".to_owned(),
    ];
    for c in &cases {
        fast("a refused chain", || {
            decode_chain("chains/a.k7x", c, song::PathRule::HOST).expect_err(c)
        });
    }
}

#[test]
fn a_chain_of_100_000_items_decodes_linearly() {
    let items: String = (0..100_000).map(|i| format!("id(\"n{i}\").extends(\"x\"),")).collect();
    let text = format!("steps([{items}])\n");
    let c = fast("100,000 items", || {
        decode_chain("chains/a.k7x", &text, song::PathRule::HOST).expect("a chain")
    });
    assert_eq!(c.steps.len(), 100_000);
}

#[test]
fn a_chain_of_100_000_subchains_decodes_linearly() {
    let subs: String = (0..100_000)
        .map(|i| format!("{{id: \"s{i}\", steps: [id(\"d{i}\").extends(\"w\")]}},"))
        .collect();
    let text = format!("steps([id(\"a\").extends(\"x\")])\nsubchains([{subs}])\n");
    let c = fast("100,000 subchains", || {
        decode_chain("chains/a.k7x", &text, song::PathRule::HOST).expect("a chain")
    });
    assert_eq!(c.subchains.map(|s| s.len()), Some(100_000));
}

#[test]
fn layout_files_that_are_hostile() {
    // Decision 103: a bad entry is the file's finding; a positions that is no object is ignored.
    for bad in [
        "positions({ a: { x: 1, y: 2, z: 3 } })\n",
        "positions({ a: { x: \"1\", y: 2 } })\n",
        "positions({ a: { x: 1e400, y: 2 } })\n",
        "positions({ \"\": { x: 1, y: 2 } })\n",
    ] {
        decode_layout("layout.k7x", bad).expect_err(bad);
    }
    assert!(
        decode_layout("layout.k7x", "positions([1])\nother(3)\n")
            .expect("ignored")
            .positions
            .is_empty()
    );
    let body: String = (0..200_000).map(|i| format!("n{i}:{{x:{i},y:0}},")).collect();
    let l = fast("200,000 positions", || {
        decode_layout("layout.k7x", &format!("positions({{{body}}})\n")).expect("a layout")
    });
    assert_eq!(l.positions.len(), 200_000);
}

fn segments_of(files: Vec<(String, String)>) -> Result<song::routing::Built, song::routing::Capped> {
    let chains = files
        .iter()
        .map(|(n, t)| {
            let c = decode_chain(&format!("chains/{n}.k7x"), t, song::PathRule::HOST)
                .unwrap_or_else(|f| panic!("{n}: {}", f.message));
            (n.clone(), std::sync::Arc::new(c))
        })
        .collect();
    let mut budget = Budget::default();
    to_segments(&chains, &|_| false, &IndexMap::new(), &mut budget)
}

fn items(prefix: &str, n: usize) -> String {
    (0..n)
        .map(|i| format!("id(\"{prefix}{i}\").extends(\"x\")"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn two_adjacent_parallel_steps_of_5000_are_past_the_budget() {
    // Decision 105: 25 million edges from 10,000 items; one error naming the file, nothing built.
    let text = format!(
        "start(true)\nsteps([id(\"h\").extends(\"x\"), [{}], [{}]])\n",
        items("a", 5000),
        items("b", 5000)
    );
    let e = fast("parallel steps", || segments_of(vec![("big".into(), text)])).expect_err("capped");
    assert_eq!(e.file, "chains/big.k7x");
}

#[test]
fn an_end_of_5000_nodes_and_5000_links_is_past_the_budget() {
    let links: String = (0..5000)
        .map(|_| "chain(\"OUT\").level(1)")
        .collect::<Vec<_>>()
        .join(",");
    let text = format!(
        "start(true)\nsteps([id(\"h\").extends(\"x\"), [{}], to([{links}])])\n",
        items("a", 5000)
    );
    assert!(fast("an end", || segments_of(vec![("big".into(), text)])).is_err());
}

#[test]
fn a_ladder_of_100_000_link_only_chains_is_capped_without_a_stack() {
    // Decision 104: the walk is an explicit stack; a ladder this deep would trap a WebAssembly module.
    let n = 100_000;
    let mut files = vec![(
        "top".to_owned(),
        "start(true)\nsteps([id(\"t\").extends(\"x\"), to([chain(\"c0\")])])\n".to_owned(),
    )];
    for i in 0..n {
        let next = if i + 1 < n {
            format!("c{}", i + 1)
        } else {
            "OUT".to_owned()
        };
        files.push((format!("c{i}"), format!("steps([to([chain(\"{next}\")])])\n")));
    }
    let r = fast("a ladder", || segments_of(files));
    assert!(r.is_err());
}

#[test]
fn a_thousand_link_only_chains_each_linking_all_the_others_are_capped() {
    let n = 1000;
    let mut files = vec![(
        "top".to_owned(),
        "start(true)\nsteps([id(\"t\").extends(\"x\"), to([chain(\"c0\")])])\n".to_owned(),
    )];
    for i in 0..n {
        let links: Vec<String> = (0..n)
            .filter(|j| *j != i)
            .map(|j| format!("chain(\"c{j}\").level(1)"))
            .collect();
        files.push((format!("c{i}"), format!("steps([to([{}])])\n", links.join(","))));
    }
    assert!(fast("all linking all", || segments_of(files)).is_err());
}

#[test]
fn a_link_only_cycle_and_100_000_subchains_are_answered() {
    let cycle = vec![
        (
            "top".to_owned(),
            "start(true)\nsteps([id(\"t\").extends(\"x\"), to([chain(\"p\")])])\n".to_owned(),
        ),
        ("p".to_owned(), "steps([to([chain(\"q\")])])\n".to_owned()),
        ("q".to_owned(), "steps([to([chain(\"p\")])])\n".to_owned()),
    ];
    let built = segments_of(cycle).expect("a cycle ends");
    assert!(built.segments.modules.expect("modules")["t"].is_empty());
    let subs: String = (0..100_000)
        .map(|i| format!("{{id: \"s{i}\", steps: [id(\"d{i}\").extends(\"w\")]}},"))
        .collect();
    let text = format!("start(true)\nsteps([id(\"a\").extends(\"x\")])\nsubchains([{subs}])\n");
    let built = fast("100,000 subchains", || segments_of(vec![("big".into(), text)])).expect("built");
    assert_eq!(built.segments.modules.expect("modules").len(), 1);
}

fn graph(pairs: Vec<(String, Vec<String>)>) -> Segments {
    Segments {
        modules: Some(pairs.into_iter().collect()),
        ..Segments::default()
    }
}

#[test]
fn derivation_stays_linear_on_a_cycle_a_fan_in_and_a_lane_on_many_ports() {
    // Decision 106: the indexes are built once.
    let n = 100_000;
    let ring = graph(
        (0..n)
            .map(|i| (format!("n{i}"), vec![format!("n{}", (i + 1) % n)]))
            .collect(),
    );
    let g = fast("a ring", || derive_chains(&ring, &|_| false));
    assert_eq!(g.len(), 1);
    let fan = graph((0..n).map(|i| (format!("n{i}"), vec!["sink".to_owned()])).collect());
    let g = fast("a fan-in", || derive_chains(&fan, &|_| false));
    assert_eq!(g.len(), n);
    let ports = graph(
        (0..1000)
            .map(|i| (format!("lane#p{i}"), vec![format!("m{i}")]))
            .collect(),
    );
    let g = fast("a lane on 1,000 ports", || derive_chains(&ports, &|x| x == "lane"));
    assert_eq!(g.keys().filter(|k| k.starts_with("lane#")).count(), 1000);
    let own = graph(vec![("a".into(), vec!["a".into()])]);
    assert_eq!(derive_chains(&own, &|_| false).len(), 1);
}

#[test]
fn three_hundred_thousand_keys_each_feeding_the_next_derive_in_time() {
    let n = 300_000;
    let chain = graph((0..n).map(|i| (format!("n{i}"), vec![format!("n{}", i + 1)])).collect());
    let g = fast("a long run", || derive_chains(&chain, &|x| x == "n0"));
    assert_eq!(g["n0"].use_steps.len(), n - 1);
}

#[test]
fn a_lane_name_with_a_windows_separator_is_refused_under_the_windows_rule_only() {
    // Decision 93: a `\` is a character of a name under the Posix rule and a way out of `tracks/` under the Windows one.
    for name in ["..\\\\..\\\\x", "a\\\\b"] {
        let text = chain(&format!("id(\"l\").track(\"{name}\")"));
        for rule in [song::PathRule::Posix, song::PathRule::Windows] {
            let r = decode_chain("chains/a.k7x", &text, rule);
            assert_eq!(r.is_err(), rule.is_windows(), "{name} under {rule:?}");
        }
    }
}

#[test]
fn a_hundred_thousand_calls_over_a_hundred_thousand_subchains_expand_in_time() {
    // Decision 104: the subchains are indexed by id, not searched once per call.
    let n = 100_000;
    let calls: String = (0..n).map(|i| format!("{{sub: \"s{i}\"}},")).collect();
    let subs: String = (0..n)
        .map(|i| format!("{{id: \"s{i}\", steps: [id(\"d{i}\").extends(\"w\")]}},"))
        .collect();
    let text = format!("start(true)\nsteps([id(\"a\").extends(\"x\"), {calls}])\nsubchains([{subs}])\n");
    let c = decode_chain("chains/big.k7x", &text, song::PathRule::HOST).expect("a chain");
    let steps = fast("100,000 calls", || c.expand_calls().len());
    assert_eq!(steps, n + 1);
}

#[test]
fn a_hundred_thousand_unlinked_subchains_of_a_file_with_no_picture_lane_are_answered_in_time() {
    // Decision 104: a file's picture lane is looked for once, not once per subchain.
    let n = 100_000;
    let subs: String = (0..n)
        .map(|i| format!("{{id: \"s{i}\", steps: [id(\"d{i}\").extends(\"w\")]}},"))
        .collect();
    let text = format!("start(true)\nsteps([{}])\nsubchains([{subs}])\n", items("a", n));
    let built = fast("100,000 unlinked subchains", || segments_of(vec![("big".into(), text)]));
    assert_eq!(built.expect("built").segments.modules.expect("modules").len(), n);
}
