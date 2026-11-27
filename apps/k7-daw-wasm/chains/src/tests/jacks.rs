// The sockets a node shows: mandatory ones up front, one spare past the last optional one taken, every output at once.

use super::*;
use serde_json::json;

fn crossfade() -> Value {
    json!([{"name": "a", "type": "audio"}, {"name": "b", "type": "audio", "mandatory": false}])
}

/// deckA flows into the crossfade, deckB lands on its `b`.
fn wired() -> Value {
    doc(
        json!({
            "deckA": {"from": "deckA", "use": [{"module": "crossfade", "inputs": crossfade(), "emits": "audio"}], "out": [{"to": "OUT", "gain": 1}]},
            "deckB": {"from": "deckB", "use": [], "out": [{"to": "crossfade#b", "gain": 1}]}
        }),
        json!(["deckA", "deckB"]),
    )
}

#[test]
fn counts_a_ported_cable_against_the_port_it_names() {
    let b = built(&wired(), json!({}));
    let xf = b.node("mod:crossfade");
    assert_eq!(
        xf["jacks"]["in"],
        json!([{"name": "a", "type": "audio", "free": false}, {"name": "b", "type": "audio", "free": false}])
    );
    assert_eq!(xf["jacks"]["out"].as_array().map(Vec::len), Some(1));
}

#[test]
fn gives_a_lane_two_outputs_audio_then_midi() {
    let b = built(&doc(json!({}), json!(["deckB"])), json!({}));
    assert_eq!(
        b.node("trk:deckB")["jacks"]["out"],
        json!([{"name": "audio", "type": "audio", "free": true}, {"name": "midi", "type": "midi", "free": true}])
    );
}

#[test]
fn gives_a_lane_an_input_socket_and_a_spare_beside_the_output_that_is_taken() {
    let b = built(&wired(), json!({}));
    let deck = b.node("trk:deckA");
    assert_eq!(deck["jacks"]["in"], json!([{"type": "audio", "free": true}]));
    assert_eq!(
        deck["jacks"]["out"],
        json!([
            {"name": "audio", "type": "audio", "free": false},
            {"type": "audio", "free": true},
            {"name": "midi", "type": "midi", "free": true}
        ])
    );
}

#[test]
fn still_leaves_a_sink_with_no_output_socket() {
    let b = built(&wired(), json!({}));
    assert_eq!(b.node("snk:OUT")["jacks"]["out"], json!([]));
    assert_eq!(b.node("snk:OUT")["jacks"]["in"].as_array().map(Vec::len), Some(1));
}

#[test]
fn leaves_a_live_input_with_no_input_socket() {
    let g = doc(
        json!({"live": {"from": "live", "use": [{"module": "IN", "emits": "audio", "monitor": "off"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!([]),
    );
    let b = built(&g, json!({}));
    let node = b.node("mod:IN");
    assert_eq!(node["jacks"]["in"], json!([]));
    assert_eq!(node["jacks"]["out"].as_array().map(Vec::len), Some(1));
}

#[test]
fn does_not_let_an_automation_arrow_occupy_an_input() {
    let g = doc_with(
        json!({"a": {"from": "t", "use": [{"module": "amp", "emits": "audio", "automations": [{"param": "Gain", "curve": "sweep"}]}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["t"]),
        json!({"curves": {"sweep": {"type": "custom"}}}),
    );
    assert_eq!(
        built(&g, json!({})).node("mod:amp")["jacks"]["in"],
        json!([{"type": "audio", "free": false}])
    );
}

#[test]
fn carries_the_port_through_layout() {
    let b = built(&wired(), json!({}));
    let laid = b.laid(json!(null), json!(null));
    let ported: Vec<Value> = list(&laid["edges"])
        .into_iter()
        .filter(|e| e["target"] == "mod:crossfade")
        .filter_map(|e| e.get("targetPort").cloned())
        .collect();
    assert_eq!(ported, [json!("b")]);
}

fn meeting() -> Value {
    doc(
        json!({
            "deckA": {"from": "deckA", "use": [], "out": [{"to": "eqA", "gain": 1}]},
            "drums": {"from": "drums", "use": [], "out": [{"to": "eqA", "gain": 1}]},
            "bass": {"from": "bass", "use": [], "out": [{"to": "eqA", "gain": 1}]},
            "eqA": {"from": "eqA", "use": [{"module": "eqA", "emits": "audio"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["deckA", "drums", "bass"]),
    )
}

#[test]
fn several_cables_into_one_port_less_node_each_get_a_socket_of_their_own() {
    let b = built(&meeting(), json!({}));
    let ins = list(&b.node("mod:eqA")["jacks"]["in"]);
    // Four, not three: a bus chain starts at its own meeting point, so its own lane is a cable too.
    assert_eq!(ins.len(), 4);
    assert!(ins.iter().all(|j| j["free"] == false));
}

#[test]
fn several_cables_are_ranked_in_the_graphs_own_order_so_a_cable_keeps_its_socket() {
    let b = built(&meeting(), json!({}));
    let slots: Vec<Value> = b
        .edges
        .iter()
        .filter(|e| e["target"] == "mod:eqA")
        .map(|e| e["targetSlot"].clone())
        .collect();
    assert_eq!(slots, [json!(0.0), json!(1.0), json!(2.0), json!(3.0)]);
}
