// A panel's widgets on EVERY tab, for whatever looks one up by its bind: a widget on a closed tab still exists.
// The page's `graph-model/panel.ts` keeps the same two rules over the objects it already holds (a lookup per frame).

use super::*;
use serde_json::json;

fn panel(v: serde_json::Value) -> Panel {
    serde_json::from_value(v).unwrap_or_default()
}

#[test]
fn every_widget_comes_shared_ones_first_then_each_tabs_in_order() {
    let p = panel(json!({
        "widgets": {"a": {"type": "knob", "col": 0, "row": 0}},
        "tabs": [
            {"name": "one", "widgets": {"b": {"type": "knob", "col": 0, "row": 0}}},
            {"name": "two", "widgets": {"c": {"type": "knob", "col": 0, "row": 0}}}
        ]
    }));
    let ids: Vec<&str> = p.all_widgets().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(Panel::default().all_widgets().count(), 0);
}

#[test]
fn a_single_tab_is_just_the_panel() {
    let one = panel(json!({"tabs": [{"name": "only", "widgets": {}}]}));
    let two = panel(json!({"tabs": [{"name": "a"}, {"name": "b"}]}));
    assert!(!one.has_tabs());
    assert!(two.has_tabs());
    assert!(!Panel::default().has_tabs());
}
