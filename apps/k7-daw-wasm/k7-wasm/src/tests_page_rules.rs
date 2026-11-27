// tests_page_rules.rs — the surfaces, nodes and library ops through the bytes the ABI carries.
// A reader would get wrong that the rules are tested here: they are tested in `chains` and `song-edit`, and this proves the wire (names, shapes, nulls).

use crate::Session;
use crate::ops::OK;
use serde_json::{Value, json};

fn ask(op: &str, args: Value) -> Value {
    let mut s = Session::new();
    let mut out = Vec::new();
    let status = s.handle(json!({ "op": op, "args": args }).to_string().as_bytes(), &mut out);
    assert_eq!(status, OK, "{op}: {}", String::from_utf8_lossy(&out));
    serde_json::from_slice(&out).unwrap_or(Value::Null)
}

#[test]
fn the_surface_rows_answer_groups_with_a_target_for_an_open_row() {
    let hello = json!({
        "device": {"name": "Studio", "api": 1, "proto": 1},
        "project": {"workspace": "band", "path": "mix.k7", "label": "band / mix.k7"},
        "surfaces": [{"id": "desk", "type": "turntables"}, {"id": "arr", "type": "timeline"}],
        "screens": [{"surface": "desk", "device": "iPad"}]
    });
    let peer = json!({"engineId": "e1", "name": "Studio", "host": "10.0.0.2", "reached": true, "trusted": true});
    let got = ask(
        "surfaces.rows",
        json!({
            "peers": [{"peer": peer, "asked": true, "hello": hello}], "selfId": null, "selfName": "This Mac",
            "drawable": ["turntables"], "versions": {"api": 1, "minProto": 1, "proto": 1}
        }),
    );
    assert_eq!(got[0]["project"], "band / mix.k7");
    assert_eq!(got[0]["rows"][0]["target"]["surface"], "desk");
    assert_eq!(got[0]["rows"][0]["alsoOn"], json!(["iPad"]));
    assert_eq!(got[0]["rows"][1]["blocked"], json!({"why": "windowOnly"}));
    assert!(got[0]["rows"][1].get("target").is_none());
}

#[test]
fn the_panes_remotes_hold_and_the_next_free_one() {
    let screens = json!([{"surface": "desk", "device": "iPad", "since": 1}]);
    let ids = json!({"timeline": "arr", "turntables": "desk", "viewport": null});
    assert_eq!(
        ask("surfaces.inUse", json!({"screens": screens, "ids": ids})),
        json!({"turntables": "iPad"})
    );
    let order = json!(["timeline", "turntables", "chains"]);
    let next = ask(
        "surfaces.nextFree",
        json!({"order": order, "from": "turntables", "blocked": ["turntables"]}),
    );
    assert_eq!(next, "chains");
    assert!(ask("surfaces.sameScreens", json!({"a": screens, "b": screens})).as_bool() == Some(true));
}

#[test]
fn a_history_row_answers_its_marks_as_lists() {
    let graph = json!({"chains": {"be.bass": {"use": [{"module": "5ee626e6"}]}}});
    let got = ask(
        "surfaces.history",
        json!({"paths": ["chains/be.bass.k7x", "tracks/da.main.k7x"], "label": "node param \"5ee626e6\"", "graph": graph}),
    );
    assert_eq!(
        got,
        json!({"lanes": ["da.main"], "clips": [], "chains": [], "modules": ["5ee626e6"]})
    );
    let none = ask(
        "surfaces.masterModules",
        json!({"mapping": {"a": {"module": "qm", "param": "x"}, "b": null}}),
    );
    assert_eq!(none, json!(["qm"]));
}

#[test]
fn a_selection_links_by_name_and_a_box_catches_by_name() {
    let node = |m: &str, x: f64| json!({"kind": "module", "module": m, "uses": "@k7-core/audio/amp", "x": x, "y": 0, "w": 100, "h": 50});
    let nodes = json!([node("a", 0.0), node("b", 200.0)]);
    let linked = ask(
        "nodes.linked",
        json!({"nodes": nodes, "selection": ["a", "b"], "of": node("a", 0.0)}),
    );
    assert_eq!(linked, json!(["b"]));
    let caught = ask(
        "nodes.inBox",
        json!({"nodes": nodes, "box": {"x0": 150, "y0": 10, "x1": 250, "y1": 20}}),
    );
    assert_eq!(caught, json!(["b"]));
    let toggled = ask(
        "nodes.toggleSelection",
        json!({"current": [], "primary": "a", "name": "b"}),
    );
    assert_eq!(toggled, json!(["a", "b"]));
}

#[test]
fn the_shown_names_and_a_groups_edge_cross_as_maps() {
    let steps = json!([{"module": "u1", "uses": "@k7-core/audio/amp", "place": "ge.trem · step 2"}, {"module": "x"}]);
    assert_eq!(
        ask("nodes.shownNames", json!({"steps": steps})),
        json!({"u1": "amp · ge.trem · step 2"})
    );
    let widgets = json!({
        "g": {"type": "group", "col": 2, "row": 0, "colSpan": 6, "rowSpan": 3, "label": "FILTER"},
        "a": {"type": "knob", "col": 2, "row": 0, "colSpan": 2, "rowSpan": 2, "bind": "Cutoff"}
    });
    assert_eq!(ask("nodes.groupEdge", json!({"widgets": widgets})), json!({"a": 12.0}));
}

#[test]
fn the_library_tree_and_its_path_rules_answer_over_the_wire() {
    let levels = json!([{"ns": "k7-core", "path": "_libraries/k7-core/modules", "exists": true, "nodes": []}]);
    let rows = ask("library.tree", json!({"levels": levels}));
    assert_eq!(rows[0]["name"], "@k7-core");
    assert_eq!(rows[0]["children"][1]["path"], "_libraries/k7-core/chains");
    assert_eq!(
        ask(
            "library.rootOf",
            json!({"levels": levels, "path": "_libraries/k7-core/modules/audio"})
        ),
        0
    );
    assert_eq!(
        ask("library.rootOf", json!({"levels": levels, "path": "song.k7"})),
        Value::Null
    );
    let file = "_libraries/k7-core/modules/audio/reverb/module.k7x";
    assert_eq!(ask("library.moduleRef", json!({"path": file})), "@k7-core/audio/reverb");
    assert_eq!(
        ask("library.fileFolder", json!({"path": "_libraries/k7-core/irs/x"})),
        "irs"
    );
    assert_eq!(ask("library.isPath", json!({"path": file})), true);
    assert_eq!(ask("library.ofPath", json!({"path": file})), "k7-core");
    assert_eq!(ask("library.readOnly", json!({"levels": levels})), json!([]));
}

// The skeleton `project surface new mastering` writes and the controls the page draws are two tables of one list.
#[cfg(feature = "verbs")]
#[test]
fn the_mastering_scaffold_lists_the_pages_controls_in_order_with_the_same_notes() {
    let skeleton = verbs::surface_skeleton("mastering").unwrap_or_default();
    let controls: Vec<(String, String)> = chains::master_controls::MASTER_CONTROLS
        .iter()
        .map(|c| (c.id.to_owned(), c.wants.to_owned()))
        .collect();
    assert_eq!(skeleton, controls);
}
