// The TypeScript tests of bindings.ts, with their inputs and expected values.

use super::*;
use serde_json::json;

fn graph(v: Value) -> BindGraph {
    serde_json::from_value(v).unwrap_or_default()
}

fn target(module: &str, param: &str) -> SurfaceTarget {
    SurfaceTarget {
        module: module.into(),
        param: param.into(),
        invert: None,
    }
}

fn mapping(v: Value) -> IndexMap<String, Option<SurfaceBinding>> {
    serde_json::from_value(v).unwrap_or_default()
}

fn controls() -> IndexSet<String> {
    ["a.one", "a.two"].iter().map(|s| (*s).to_owned()).collect()
}

fn eq_graph() -> BindGraph {
    graph(json!({"chains": {"bus": {"use": [
        {"module": "eqA", "pluginId": "bus:eqA", "ui": {"widgets": {"g": {"bind": "Low", "min": -20, "max": 20}}}}
    ]}}}))
}

#[test]
fn a_module_is_found_by_the_name_the_chains_file_uses_and_the_first_use_wins() {
    let g = graph(json!({"chains": {
        "bus": {"use": [{"module": "eqA"}, {"module": "filterA"}]},
        "one": {"from": "t1", "use": [{"module": "fade", "pluginId": "one:fade"}]},
        "two": {"from": "t2", "use": [{"module": "fade", "pluginId": "two:fade"}]},
    }}));
    let steps = steps_by_module(&g);
    assert_eq!(
        steps.keys().copied().collect::<Vec<_>>(),
        vec!["eqA", "filterA", "fade"]
    );
    assert_eq!(steps["fade"].step.plugin_id.as_deref(), Some("one:fade"));
    assert_eq!(steps["fade"].track, Some("t1"));
    assert!(steps_by_module(&BindGraph::default()).is_empty());
}

#[test]
fn a_widget_is_found_by_its_bind_on_any_tab_and_a_missing_panel_is_not_fatal() {
    let g = graph(json!({"chains": {"bus": {"use": [
        {"module": "eqA", "ui": {"widgets": {"low": {"bind": "Low-shelf gain", "min": -20, "max": 20}},
                                  "tabs": [{"name": "t", "widgets": {"hi": {"bind": "High", "max": 3}}}]}},
        {"module": "m"},
    ]}}}));
    let steps = steps_by_module(&g);
    assert_eq!(
        widget_for(steps["eqA"].step, "Low-shelf gain").and_then(|w| w["min"].as_f64()),
        Some(-20.0)
    );
    assert_eq!(
        widget_for(steps["eqA"].step, "High").and_then(|w| w["max"].as_f64()),
        Some(3.0)
    );
    assert!(widget_for(steps["eqA"].step, "Mid gain 1").is_none());
    assert!(widget_for(steps["m"].step, "anything").is_none());
}

#[test]
fn a_plugless_module_binds_by_its_track_under_track_and_is_refused_under_refuse() {
    let g = graph(json!({"chains": {
        "deckA": {"from": "deckA", "use": [{"module": "tempoA"}]},
        "bus": {"use": [{"module": "lonely"}]},
    }}));
    let t = resolve_one(&g, &target("tempoA", "Target BPM"), PluglessPolicy::Track);
    assert_eq!(t.as_ref().map(|t| t.track.as_deref()), Ok(Some("deckA")));
    assert_eq!(t.map(|t| t.plugin_id), Ok(None));
    let refused = resolve_one(&g, &target("tempoA", "x"), PluglessPolicy::Refuse);
    assert!(refused.is_err_and(|m| m.contains("loads no plugin")));
    for p in [PluglessPolicy::Track, PluglessPolicy::Refuse] {
        assert!(resolve_one(&g, &target("lonely", "x"), p).is_err());
    }
    let missing = resolve_one(&g, &target("nope", "x"), PluglessPolicy::Track);
    assert!(missing.is_err_and(|m| m.contains("no module") && m.contains("nope")));
}

#[test]
fn a_line_resolves_to_its_plugin_and_its_widget() {
    let m = mapping(json!({"a.one": {"module": "eqA", "param": "Low"}}));
    let r = resolve_mapping(&m, &eq_graph(), &controls(), "du pupitre", PluglessPolicy::Track, false);
    assert!(r.problems.is_empty());
    assert_eq!(r.rows[0].target.plugin_id.as_deref(), Some("bus:eqA"));
    assert_eq!(
        r.rows[0].target.widget.as_ref().and_then(|w| w["min"].as_f64()),
        Some(-20.0)
    );
}

#[test]
fn an_unknown_control_is_reported_in_the_callers_own_words() {
    let m = mapping(json!({"nope": {"module": "eqA", "param": "Low"}}));
    let r = resolve_mapping(&m, &eq_graph(), &controls(), "du pupitre", PluglessPolicy::Track, false);
    assert!(r.problems[0].message.contains("pupitre"));
    let r = resolve_mapping(
        &m,
        &eq_graph(),
        &controls(),
        "de cette surface",
        PluglessPolicy::Refuse,
        false,
    );
    assert!(r.problems[0].message.contains("cette surface"));
}

#[test]
fn a_control_named_but_not_wired_is_skipped_without_a_word() {
    let r = resolve_mapping(
        &mapping(json!({"a.one": null})),
        &eq_graph(),
        &controls(),
        "x",
        PluglessPolicy::Track,
        false,
    );
    assert!(r.rows.is_empty() && r.problems.is_empty());
}

#[test]
fn invert_is_carried_only_when_set_and_the_line_is_handed_back() {
    let on = mapping(
        json!({"a.one": {"module": "eqA", "param": "Low", "invert": true, "also": {"module": "eqA", "param": "High"}}}),
    );
    let r = resolve_mapping(&on, &eq_graph(), &controls(), "x", PluglessPolicy::Track, false);
    assert_eq!(r.rows[0].target.invert, Some(true));
    assert_eq!(r.rows[0].binding.also.as_ref().map(|a| a.param.as_str()), Some("High"));
    let off = mapping(json!({"a.one": {"module": "eqA", "param": "Low"}}));
    let r = resolve_mapping(&off, &eq_graph(), &controls(), "x", PluglessPolicy::Track, false);
    assert_eq!(r.rows[0].target.invert, None);
}

#[test]
fn every_bad_line_is_reported_not_just_the_first() {
    let m = mapping(json!({"nope": {"module": "eqA", "param": "L"}, "a.one": {"module": "gone", "param": "L"}}));
    let r = resolve_mapping(&m, &eq_graph(), &controls(), "x", PluglessPolicy::Track, false);
    assert_eq!(r.problems.len(), 2);
}

#[test]
fn a_second_target_resolves_the_same_way_and_its_failure_comes_after_every_line_s_own() {
    let m = mapping(json!({
        "a.one": {"module": "eqA", "param": "Low", "also": {"module": "gone", "param": "x"}},
        "a.two": {"module": "missing", "param": "Low"},
    }));
    let r = resolve_mapping(&m, &eq_graph(), &controls(), "x", PluglessPolicy::Track, true);
    assert!(r.rows[0].also.is_none());
    assert_eq!(r.problems.len(), 2);
    assert!(r.problems[0].message.starts_with("no module"));
    assert!(r.problems[1].message.starts_with("the second module “gone”"));
    let ok = mapping(json!({"a.one": {"module": "eqA", "param": "Low", "also": {"module": "eqA", "param": "High"}}}));
    let r = resolve_mapping(&ok, &eq_graph(), &controls(), "x", PluglessPolicy::Track, true);
    assert_eq!(r.rows[0].also.as_ref().map(|a| a.param.as_str()), Some("High"));
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_control_is_unmapped_unless_a_line_names_a_module_for_it() {
    let all = ids(&["a.one", "a.two"]);
    assert_eq!(unmapped_controls(&all, &IndexMap::new()), all);
    assert_eq!(unmapped_controls(&all, &mapping(json!({"a.one": null}))), all);
    let wired = mapping(json!({"a.one": {"module": "absent", "param": "p"}, "a.two": {"module": "m", "param": "p"}}));
    assert!(unmapped_controls(&all, &wired).is_empty());
}
