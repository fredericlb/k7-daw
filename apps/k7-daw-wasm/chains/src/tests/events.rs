// Event plugs: what a module announces and what an envelope listens for, the pair a loop on the graph.

use super::*;
use serde_json::json;

fn env() -> Value {
    json!({"type": "adsr"})
}

fn lfo() -> Value {
    json!({"type": "lfo", "shape": "sine"})
}

fn auto() -> Value {
    json!({"param": "Filter Freq", "curve": "env1"})
}

/// A synth lane whose osc has its filter swept by `curve`.
fn graph_of(curve: Value, automation: Value) -> Value {
    doc_with(
        json!({"lead": {"from": "synth", "use": [
            {"module": "osc", "category": "instruments", "events": ["note-on"], "automations": [automation]}
        ], "out": [{"to": "OUT"}]}}),
        json!(["synth"]),
        json!({"curves": {"env1": curve}}),
    )
}

fn curve_node(g: &Value) -> Value {
    built(g, json!({}))
        .of_kind("curve")
        .first()
        .map(|n| (*n).clone())
        .expect("a curve card")
}

fn event_edge(g: &Value) -> Option<Value> {
    built(g, json!({})).edges.into_iter().find(|e| e["signal"] == "event")
}

#[test]
fn a_module_carries_its_events_onto_the_node_for_the_sockets_down_its_right_edge() {
    assert_eq!(
        built(&graph_of(env(), auto()), json!({})).node("mod:osc")["events"],
        json!(["note-on"])
    );
}

#[test]
fn a_module_that_announces_nothing_says_nothing() {
    let mut g = graph_of(env(), auto());
    if let Some(step) = g["chains"]["lead"]["use"][0].as_object_mut() {
        step.remove("events");
    }
    assert!(built(&g, json!({})).node("mod:osc")["events"].is_null());
}

#[test]
fn an_envelope_has_an_event_socket() {
    assert_eq!(curve_node(&graph_of(env(), auto()))["eventIn"], true);
}

#[test]
fn a_curve_of_song_time_has_none_nothing_fires_it() {
    assert_eq!(curve_node(&graph_of(lfo(), auto()))["eventIn"], false);
}

fn with_spare(spare: Value) -> Value {
    let mut g = graph_of(env(), auto());
    g["curves"]["spare"] = spare;
    g
}

#[test]
fn a_curve_nothing_binds_is_drawn_anyway_a_card_waiting_to_be_wired() {
    let b = built(&with_spare(json!({"type": "lfo", "shape": "saw"})), json!({}));
    let free = b.node("crv:spare");
    assert_eq!(
        (&free["kind"], &free["label"], &free["sub"]),
        (&json!("curve"), &json!("spare"), &json!("lfo saw"))
    );
}

#[test]
fn a_free_curve_has_no_binding_in_its_id_which_tells_it_from_an_instance() {
    let ids = built(&with_spare(json!({"type": "adsr"})), json!({})).ids();
    let curves: Vec<&String> = ids.iter().filter(|i| i.starts_with("crv:")).collect();
    assert!(curves.contains(&&"crv:spare".to_owned()));
    assert!(curves.iter().any(|i| i.starts_with("crv:env1@")));
    assert!(!curves.contains(&&"crv:env1".to_owned()));
}

#[test]
fn a_free_envelope_listens_too() {
    assert_eq!(
        built(&with_spare(json!({"type": "adsr"})), json!({})).node("crv:spare")["eventIn"],
        true
    );
}

#[test]
fn several_instances_of_one_curve_each_get_their_own_node_fired_by_their_own_module() {
    let mut g = graph_of(env(), auto());
    if let Some(steps) = g["chains"]["lead"]["use"].as_array_mut() {
        steps.push(
            json!({"module": "drums", "category": "instruments", "events": ["note-on"],
                          "automations": [{"param": "Level", "curve": "env1", "trigger": "drums"}]}),
        );
    }
    let b = built(&g, json!({}));
    let curves = b.of_kind("curve");
    assert_eq!(curves.len(), 2);
    assert_eq!(
        curves.iter().map(|n| text(&n["label"])).collect::<Vec<_>>(),
        ["env1", "env1"]
    );
    assert_eq!(curves.iter().map(|n| text(&n["id"])).collect::<HashSet<_>>().len(), 2);
    let mut fired: Vec<String> = b
        .edges
        .iter()
        .filter(|e| e["signal"] == "event")
        .map(|e| text(&e["source"]))
        .collect();
    fired.sort();
    assert_eq!(fired, ["mod:drums", "mod:osc"]);
}

#[test]
fn the_cable_that_fires_a_curve_runs_from_the_announcing_module_into_it() {
    let g = graph_of(env(), auto());
    let e = event_edge(&g).expect("an event cable");
    assert_eq!((&e["source"], &e["sourcePort"]), (&json!("mod:osc"), &json!("note-on")));
    assert_eq!(e["target"], curve_node(&g)["id"]);
}

#[test]
fn the_cable_defaults_to_the_module_being_automated_as_the_compiler_does() {
    assert_eq!(
        event_edge(&graph_of(env(), auto())).map(|e| e["source"].clone()),
        Some(json!("mod:osc"))
    );
}

#[test]
fn the_cable_follows_trigger_when_the_binding_names_another_module() {
    let g = graph_of(
        env(),
        json!({"param": "Filter Freq", "curve": "env1", "trigger": "drums"}),
    );
    assert_eq!(event_edge(&g).map(|e| e["source"].clone()), Some(json!("mod:drums")));
}

#[test]
fn the_cable_is_not_drawn_for_a_curve_of_song_time() {
    let g = graph_of(lfo(), auto());
    assert!(event_edge(&g).is_none());
    // The automation edge still is: an LFO drives its param without being fired.
    assert!(
        built(&g, json!({}))
            .edges
            .iter()
            .any(|e| e["automation"]["param"] == "Filter Freq")
    );
}
