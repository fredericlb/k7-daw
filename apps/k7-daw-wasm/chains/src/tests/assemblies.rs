// A box closed and a box open over the same graph: a closed box hides its members and wears their boundary cables on its plugs.

use super::*;
use crate::assembly::assembly_id;
use crate::row_seams::jack_ref;
use serde_json::json;

fn the_box() -> Value {
    json!({
        "uses": "@k7-core/turntable", "label": "Turntable", "members": ["trimA", "fadeA"],
        "inputs": [{"name": "in", "type": "audio", "node": "trimA"}],
        "outputs": [{"name": "mix", "type": "audio", "node": "fadeA"}],
        "params": {"Trim": {"node": "trimA", "param": "Trim"}}
    })
}

/// A lane through two modules straight to the device, the two declared one box.
fn gtr_with(b: Value) -> Value {
    doc_with(
        json!({"gtr": {"from": "gtr", "use": [{"module": "trimA"}, {"module": "fadeA"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["gtr"]),
        json!({"assemblies": {"turntableA": b}}),
    )
}

fn fold(open: &[&str]) -> Built {
    folded(&gtr_with(the_box()), open)
}

fn assembly_card(b: &Built) -> Value {
    b.of_kind("assembly").first().map(|n| (*n).clone()).expect("a box")
}

#[test]
fn a_closed_box_draws_instead_of_its_members() {
    let b = fold(&[]);
    assert!(b.nodes.iter().all(|n| n["module"] != "trimA" && n["module"] != "fadeA"));
    let card = assembly_card(&b);
    assert_eq!(card["id"], json!(assembly_id("turntableA")));
    assert_eq!(card["label"], "Turntable");
    assert_eq!(card["module"], "turntableA");
}

#[test]
fn a_closed_box_wears_the_promoted_params_as_knobs_bound_by_their_promoted_name() {
    let card = assembly_card(&fold(&[]));
    let binds: Vec<Value> = card["ui"]["widgets"]
        .as_object()
        .map(|w| w.values().map(|x| x["bind"].clone()).collect())
        .unwrap_or_default();
    assert_eq!(binds, [json!("Trim")]);
    assert_eq!(
        card["assemblyOf"]["params"]["Trim"],
        json!({"node": "trimA", "param": "Trim"})
    );
}

#[test]
fn a_closed_box_takes_the_boundary_cables_onto_its_plugs_and_hides_the_one_inside() {
    let b = fold(&[]);
    let id = assembly_id("turntableA");
    let ends: Vec<(Value, Value, Value, Value)> = b
        .edges
        .iter()
        .map(|e| {
            (
                e["source"].clone(),
                e["sourcePort"].clone(),
                e["target"].clone(),
                e["targetPort"].clone(),
            )
        })
        .collect();
    assert!(ends.contains(&(json!("trk:gtr"), Value::Null, json!(id), json!("in"))));
    assert!(ends.contains(&(json!(id), json!("mix"), json!("snk:OUT"), Value::Null)));
    assert!(b.edges.iter().all(|e| e["source"] != e["target"]));
    assert_eq!(b.edges.len(), 2);
}

#[test]
fn a_closed_box_shows_a_socket_per_plug_and_none_where_the_template_declares_none() {
    let card = assembly_card(&fold(&[]));
    let names = |v: &Value| -> Vec<Value> { list(v).iter().map(|j| j["name"].clone()).collect() };
    assert_eq!(names(&card["jacks"]["in"]), [json!("in")]);
    assert_eq!(names(&card["jacks"]["out"]), [json!("mix")]);
    let mut bare = the_box();
    bare["inputs"] = json!([]);
    let source = assembly_card(&folded(&gtr_with(bare), &[]));
    assert_eq!(source["jacks"]["in"], json!([]));
}

#[test]
fn a_closed_box_answers_a_plug_with_the_members_address_and_its_bare_name_with_its_own() {
    let b = fold(&[]);
    let card = b.typed(&assembly_id("turntableA"));
    assert_eq!(jack_ref(card, Some("mix")), "fadeA");
    assert_eq!(jack_ref(card, Some("in")), "trimA");
    assert_eq!(jack_ref(card, None), "turntableA");
}

#[test]
fn an_open_box_keeps_its_members_tagged_and_takes_no_plug_of_its_own() {
    let b = fold(&["turntableA"]);
    let mut members: Vec<String> = b
        .nodes
        .iter()
        .filter(|n| n["assembly"] == "turntableA")
        .map(|n| text(&n["module"]))
        .collect();
    members.sort();
    assert_eq!(members, ["fadeA", "trimA"]);
    let card = assembly_card(&b);
    assert_eq!(card["assemblyOf"]["open"], true);
    assert_eq!(card["jacks"], json!({"in": [], "out": []}));
    assert_eq!(b.edges.len(), 3);
}

fn inside(l: &Value) -> Vec<Value> {
    list(&l["nodes"])
        .into_iter()
        .filter(|n| n["assembly"] == "turntableA")
        .collect()
}

#[test]
fn an_open_box_lays_its_members_out_inside_the_frame() {
    let laid = fold(&["turntableA"]).laid(json!(null), json!(null));
    let nodes = list(&laid["nodes"]);
    let frame = nodes.iter().find(|n| n["kind"] == "assembly").expect("a frame");
    let f = |n: &Value, k: &str| num(&n[k]);
    for m in inside(&laid) {
        assert!(f(&m, "x") >= f(frame, "x") && f(&m, "y") >= f(frame, "y"));
        assert!(f(&m, "x") + f(&m, "w") <= f(frame, "x") + f(frame, "w"));
        assert!(f(&m, "y") + f(&m, "h") <= f(frame, "y") + f(frame, "h"));
    }
}

#[test]
fn an_open_box_keeps_every_cable_drawable_the_members_own_included() {
    let laid = fold(&["turntableA"]).laid(json!(null), json!(null));
    let ids: HashSet<String> = inside(&laid).iter().map(|n| text(&n["id"])).collect();
    let edges = list(&laid["edges"]);
    // The renderer skips a cable with fewer than two points, so a nested pass handing back none drew no cable.
    assert!(edges.iter().all(|e| e["points"].as_array().map_or(0, Vec::len) >= 2));
    let internal: Vec<&Value> = edges
        .iter()
        .filter(|e| ids.contains(&text(&e["source"])) && ids.contains(&text(&e["target"])))
        .collect();
    assert_eq!(internal.len(), 1);
}

#[test]
fn an_open_box_frames_every_member_and_nothing_else() {
    let b = fold(&["turntableA"]);
    let laid = b.laid(json!(null), json!(null));
    let ids: HashSet<String> = list(&laid["nodes"]).iter().map(|n| text(&n["id"])).collect();
    assert_eq!(ids.len(), b.nodes.len());
    assert_eq!(list(&laid["nodes"]).len(), b.nodes.len());
}

#[test]
fn marks_the_cables_inside_a_box_and_only_those() {
    let b = fold(&["turntableA"]);
    assert_eq!(b.edges.iter().filter(|e| e["assembly"] == "turntableA").count(), 1);
    assert_eq!(b.edges.iter().filter(|e| e["assembly"].is_null()).count(), 2);
}

#[test]
fn marks_nothing_while_every_box_is_closed() {
    assert!(fold(&[]).edges.iter().all(|e| e["assembly"].is_null()));
}

#[test]
fn carries_the_mark_through_the_layout() {
    let laid = fold(&["turntableA"]).laid(json!(null), json!(null));
    assert_eq!(
        list(&laid["edges"])
            .iter()
            .filter(|e| e["assembly"] == "turntableA")
            .count(),
        1
    );
}

fn pinned(open: &[&str]) -> Value {
    fold(open).laid(
        json!({"gtr": {"x": 0, "y": 0}, "OUT": {"x": 40, "y": 900}}),
        json!(null),
    )
}

fn overlap(a: &Value, b: &Value) -> bool {
    let f = |n: &Value, k: &str| num(&n[k]);
    f(a, "x") < f(b, "x") + f(b, "w")
        && f(a, "x") + f(a, "w") > f(b, "x")
        && f(a, "y") < f(b, "y") + f(b, "h")
        && f(a, "y") + f(a, "h") > f(b, "y")
}

#[test]
fn making_room_steps_the_cards_in_the_way_aside_and_says_where_they_belong() {
    let laid = pinned(&["turntableA"]);
    let nodes = list(&laid["nodes"]);
    let frame = nodes.iter().find(|n| n["assemblyOf"]["open"] == true).expect("a frame");
    let others: Vec<&Value> = nodes
        .iter()
        .filter(|n| n["assembly"].is_null() && n["kind"] != "assembly")
        .collect();
    assert!(others.iter().all(|n| !overlap(n, frame)));
    for n in others.iter().filter(|n| n["pushed"].is_object()) {
        assert_ne!(n["pushed"], json!({"x": n["x"], "y": n["y"]}));
    }
}

#[test]
fn making_room_puts_every_one_back_when_the_box_closes() {
    let (open, shut) = (pinned(&["turntableA"]), pinned(&[]));
    let at: std::collections::HashMap<String, (f64, f64)> = list(&shut["nodes"])
        .iter()
        .map(|n| (text(&n["id"]), (num(&n["x"]), num(&n["y"]))))
        .collect();
    for n in list(&open["nodes"]) {
        if n["assembly"].is_string() || n["kind"] == "assembly" {
            continue;
        }
        let belongs = if n["pushed"].is_object() {
            (num(&n["pushed"]["x"]), num(&n["pushed"]["y"]))
        } else {
            (num(&n["x"]), num(&n["y"]))
        };
        assert_eq!(Some(&belongs), at.get(&text(&n["id"])));
    }
}

#[test]
fn making_room_leaves_a_graph_with_nothing_in_the_way_untouched() {
    let spread = fold(&["turntableA"]).laid(
        json!({"gtr": {"x": 0, "y": 0}, "OUT": {"x": 9000, "y": 9000}}),
        json!(null),
    );
    assert_eq!(
        list(&spread["nodes"])
            .iter()
            .filter(|n| n["pushed"].is_object())
            .count(),
        0
    );
}

#[test]
fn a_graph_declaring_no_box_is_handed_back_as_built() {
    let mut bare = gtr_with(the_box());
    if let Some(o) = bare.as_object_mut() {
        o.remove("assemblies");
    }
    let (plain, folded_open) = (built(&bare, json!({})), folded(&bare, &[]));
    assert_eq!((plain.nodes, plain.edges), (folded_open.nodes, folded_open.edges));
}
