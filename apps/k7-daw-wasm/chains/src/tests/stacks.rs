// The stack view: a column per lane, edge to edge and as wide as its widest card, the inputs and outputs full-width bars.

use super::*;
use serde_json::json;

/// Two lanes, one with two modules, a live input feeding the first, a limiter on the master run.
fn g() -> Value {
    doc(
        json!({
            "lead": {"from": "melody", "use": [{"module": "synth", "ui": {"cols": 6, "rows": 2, "widgets": {}}}, {"module": "verb"}],
                     "out": [{"to": "MIX", "gain": 1}]},
            "low": {"from": "bass", "use": [{"module": "amp"}], "out": [{"to": "MIX", "gain": 1}]},
            "mic": {"from": "", "use": [{"module": "IN", "monitor": "on"}], "out": [{"to": "melody", "gain": 1}]},
            "master": {"from": "MIX", "use": [{"module": "limiter"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["melody", "bass"]),
    )
}

fn laid() -> Vec<Value> {
    list(&built(&g(), json!({})).stacked(&g(), &["melody", "bass"])["nodes"])
}

fn f(n: &Value, k: &str) -> f64 {
    num(&n[k])
}

#[test]
fn puts_each_lanes_modules_under_its_own_card_in_the_lanes_order_with_no_air_between_them() {
    let l = laid();
    let (lane, synth, verb) = (module(&l, "melody"), module(&l, "synth"), module(&l, "verb"));
    assert_eq!(f(synth, "x"), f(lane, "x"));
    assert_eq!(f(synth, "y"), f(lane, "y") + f(lane, "h"));
    assert_eq!(f(verb, "y"), f(synth, "y") + f(synth, "h"));
    assert!(f(module(&l, "bass"), "x") > f(lane, "x") + f(lane, "w"));
    assert_eq!(f(module(&l, "amp"), "x"), f(module(&l, "bass"), "x"));
}

#[test]
fn makes_every_card_of_a_column_as_wide_as_its_widest() {
    let l = laid();
    let col = ["melody", "synth", "verb"];
    let widths: Vec<f64> = col.iter().map(|m| f(module(&l, m), "w")).collect();
    assert!(widths.windows(2).all(|w| w[0] == w[1]));
    let b = built(&g(), json!({}));
    let widest = b
        .nodes
        .iter()
        .filter(|n| col.contains(&text(&n["module"]).as_str()))
        .map(|n| f(n, "w"))
        .fold(f64::MIN, f64::max);
    assert_eq!(widths[0], widest);
}

#[test]
fn draws_an_input_over_the_lane_it_feeds_as_wide_as_that_lanes_column() {
    let l = laid();
    let (input, lane) = (module(&l, "IN"), module(&l, "melody"));
    assert_eq!(input["bar"], true);
    assert_eq!((f(input, "x"), f(input, "w")), (f(lane, "x"), f(lane, "w")));
    assert!(f(input, "y") + f(input, "h") < f(lane, "y"));
    // Every lane's card still on one line, whether an input stands over it or not.
    assert_eq!(f(module(&l, "bass"), "y"), f(lane, "y"));
}

#[test]
fn gives_the_mix_the_last_column_the_master_run_and_out_stacked_under_it_like_any_module() {
    let l = laid();
    let sink = |label: &str| {
        l.iter()
            .find(|n| n["kind"] == "sink" && n["label"] == label)
            .expect("a sink")
            .clone()
    };
    let (mix, out, limiter) = (sink("MIX"), sink("OUT"), module(&l, "limiter"));
    assert!(mix["bar"].is_null() && out["bar"].is_null());
    assert_eq!(f(&mix, "y"), f(module(&l, "melody"), "y"));
    assert_eq!(f(&mix, "x"), l.iter().map(|n| f(n, "x")).fold(f64::MIN, f64::max));
    assert_eq!((f(limiter, "x"), f(&out, "x")), (f(&mix, "x"), f(&mix, "x")));
    assert_eq!(f(limiter, "y"), f(&mix, "y") + f(&mix, "h"));
    assert_eq!(f(&out, "y"), f(limiter, "y") + f(limiter, "h"));
    // OUT as wide as its column, not the whole stack.
    assert_eq!(f(&out, "w"), f(&mix, "w"));
}

fn fork() -> Value {
    doc(
        json!({
            "lead": {"from": "melody", "use": [{"module": "synth"}], "out": [{"to": "wet", "gain": 1}, {"to": "space", "gain": 1}]},
            "wet": {"use": [{"module": "dly"}], "out": [{"to": "MIX", "gain": 1}]},
            "space": {"use": [{"module": "rev"}], "out": [{"to": "MIX", "gain": 1}]}
        }),
        json!(["melody"]),
    )
}

fn forked() -> (Built, Value) {
    let b = built(&fork(), json!({}));
    let l = b.stacked(&fork(), &["melody"]);
    (b, l)
}

#[test]
fn a_fork_sends_the_cable_that_skips_cards_round_the_columns_side_and_only_that_one() {
    let (b, l) = forked();
    let from_synth: Vec<String> = b
        .edges
        .iter()
        .filter(|e| e["source"] == "mod:synth")
        .map(|e| text(&e["id"]))
        .collect();
    assert_eq!(from_synth.len(), 2);
    let sides: Vec<Value> = list(&l["sides"])
        .into_iter()
        .filter(|s| from_synth.contains(&text(&s[0])))
        .collect();
    // One target is stacked right under the synth and needs no lane; the other is further down.
    assert_eq!(sides.len(), 1);
    assert_eq!(sides[0][1], json!(0.0));
}

#[test]
fn a_fork_gives_the_forking_card_its_output_sockets_back_and_no_other_card_any() {
    let (_, l) = forked();
    let nodes = list(&l["nodes"]);
    let synth = module(&nodes, "synth");
    assert!(!list(&synth["jacks"]["out"]).is_empty());
    assert_eq!(synth["jacks"]["in"], json!([]));
    assert!(module(&nodes, "dly")["jacks"].is_null());
}
