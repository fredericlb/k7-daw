// bindings.rs — the desk's controls and their join to the project: what must be wired, what a binding resolves to and what it sends.
// A reader would get wrong whose cases these are: controls.test.ts, mapping.test.ts and mixBindings.test.ts, the graph spelled as JSON.

use chains::bindings::BindGraph;
use indexmap::IndexMap;
use serde_json::{Value, json};
use turntables::bindings::{
    DeskBinding, Mapping, offline_publish, required_controls, resolve_bindings, stretch_targets, unmapped, value_for,
};
use turntables::controls::mix_controls;
use turntables::vocab::{DECKS, DeckTempo, DeckTempos};

fn ids(cs: &[turntables::controls::MixControl]) -> Vec<String> {
    cs.iter().map(|c| c.id.clone()).collect()
}

fn mapping(v: Value) -> Mapping {
    serde_json::from_value(v).unwrap()
}

/// One chain from `deckA` holding the steps given, each with an optional widget bound to `Mix`.
fn graph(steps: Value) -> BindGraph {
    serde_json::from_value(json!({ "chains": { "deckA": { "from": "deckA", "use": steps } } })).unwrap()
}

fn step(module: &str, plugin: Option<&str>, widget: Option<Value>) -> Value {
    let mut s = json!({ "module": module });
    if let Some(p) = plugin {
        s["pluginId"] = json!(p);
    }
    if let Some(w) = widget {
        let mut w = w;
        w["bind"] = json!("Mix");
        s["ui"] = json!({ "widgets": { "w": w } });
    }
    s
}

const XFADE: &str = r#"{ "crossfade": { "module": "crossfade", "param": "Mix" } }"#;

fn xfade() -> Mapping {
    mapping(serde_json::from_str(XFADE).unwrap())
}

fn bound(widget: Option<Value>) -> DeskBinding {
    let g = graph(json!([step("crossfade", Some("deckA:crossfade"), widget)]));
    resolve_bindings(&xfade(), &g).bindings.remove(0)
}

#[test]
fn every_control_has_one_id_and_a_third_deck_would_get_its_own() {
    let all = ids(&mix_controls());
    let mut sorted = all.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), all.len());
    assert_eq!(all.first().map(String::as_str), Some("crossfade"));
    assert_eq!(all.last().map(String::as_str), Some("transport"));
    for d in DECKS {
        assert!(all.iter().filter(|i| i.starts_with(&format!("{d}."))).count() >= 12);
    }
}

#[test]
fn the_ranges_are_the_modules_own_with_flat_in_the_middle() {
    let c = |id: &str| mix_controls().into_iter().find(|c| c.id == id).unwrap();
    assert_eq!(c("deckA.tempo").range, [10.0, 300.0]);
    assert_eq!(c("deckA.eqLow").range, [-20.0, 20.0]);
    assert_eq!(c("deckB.filter").range, [-100.0, 100.0]);
    assert_eq!((c("deckA.stemBass").range, c("deckA.stemBass").home), ([0.0, 2.0], 1.0));
    assert_eq!(c("crossfade").home, 0.5);
    assert_eq!(c("deckA.eqMid").wants.as_deref(), Some("eq"));
}

#[test]
fn the_required_controls_are_the_crossfade_and_seven_per_deck() {
    let required = ids(&required_controls());
    assert_eq!(required.len(), 1 + DECKS.len() * 7);
    for d in DECKS {
        for s in ["tempo", "eqLow", "eqMid", "eqHigh", "filter", "trim", "level"] {
            assert!(required.contains(&format!("{d}.{s}")));
        }
        // The echo is optional wiring, and a button that names no module param is an action of the app.
        assert!(!required.contains(&format!("{d}.echo")) && !required.contains(&format!("{d}.sync")));
    }
    assert!(!required.contains(&"transport".to_owned()));
}

#[test]
fn what_is_missing_is_exactly_what_the_mapping_does_not_name() {
    let required = ids(&required_controls());
    assert_eq!(ids(&unmapped(&Mapping::new())), required);
    let mut full: IndexMap<String, Value> = required
        .iter()
        .map(|i| (i.clone(), json!({ "module": "m", "param": "p" })))
        .collect();
    assert!(unmapped(&mapping(json!(full))).is_empty());
    full.shift_remove("deckA.filter");
    full.insert("deckA.echo".to_owned(), json!({ "module": "m", "param": "p" }));
    assert_eq!(ids(&unmapped(&mapping(json!(full)))), ["deckA.filter"]);
    // A null line is a key that is there but not wired.
    assert_eq!(ids(&unmapped(&mapping(json!({ "crossfade": null })))), required);
}

#[test]
fn a_binding_finds_its_plugin_and_widget() {
    let g = graph(json!([step(
        "crossfade",
        Some("deckA:crossfade"),
        Some(json!({ "min": 0, "max": 1 }))
    )]));
    let done = resolve_bindings(&xfade(), &g);
    assert!(done.problems.is_empty());
    let b = &done.bindings[0];
    assert_eq!(
        (b.target.plugin_id.as_deref(), b.control.id.as_str()),
        (Some("deckA:crossfade"), "crossfade")
    );
    assert!(b.target.widget.is_some());
}

#[test]
fn what_cannot_be_resolved_is_named_not_dropped() {
    let g = graph(json!([step("crossfade", Some("x"), None)]));
    let typo = mapping(json!({ "deckA.pitch": { "module": "crossfade", "param": "Mix" } }));
    let done = resolve_bindings(&typo, &g);
    assert!(done.bindings.is_empty() && done.problems[0].message.contains("of the desk"));
    let ghost = mapping(json!({ "crossfade": { "module": "crossfad", "param": "Mix" } }));
    assert!(resolve_bindings(&ghost, &g).problems[0].message.contains("crossfad"));
    // Before the first compile there is no graph: every line is unresolvable.
    assert_eq!(resolve_bindings(&xfade(), &BindGraph::default()).problems.len(), 1);
}

#[test]
fn a_module_with_no_plugin_is_bound_by_its_track_and_one_with_neither_is_named() {
    let g = graph(json!([step("crossfade", None, None)]));
    let b = &resolve_bindings(&xfade(), &g).bindings[0];
    assert_eq!(
        (b.target.plugin_id.as_deref(), b.target.track.as_deref()),
        (None, Some("deckA"))
    );
    let orphan: BindGraph = serde_json::from_value(
        json!({ "chains": { "deckA": { "from": "", "use": [step("crossfade", None, None)] } } }),
    )
    .unwrap();
    let done = resolve_bindings(&xfade(), &orphan);
    assert!(done.bindings.is_empty() && done.problems[0].message.contains("to move"));
}

fn tempos() -> DeckTempos {
    let t = |natural, target| DeckTempo {
        natural,
        seconds: 60.0,
        target,
        phase: None,
        keylock: None,
    };
    IndexMap::from([
        ("deckA".to_owned(), t(120.0, 126.0)),
        ("deckB".to_owned(), t(90.0, 90.0)),
    ])
}

fn two_decks() -> BindGraph {
    serde_json::from_value(json!({ "chains": {
        "deckA": { "from": "deckA", "use": [{ "module": "tempoA" }] },
        "deckB": { "from": "deckB", "use": [{ "module": "tempoB" }] },
    } }))
    .unwrap()
}

#[test]
fn a_tempo_fader_stretches_the_track_its_module_draws_from() {
    let both = mapping(json!({
        "deckA.tempo": { "module": "tempoA", "param": "Target BPM" },
        "deckB.tempo": { "module": "tempoB", "param": "Target BPM" },
    }));
    let b = resolve_bindings(&both, &two_decks()).bindings;
    assert_eq!(stretch_targets(&b, &tempos(), &[]), tempos());
    // A line pointed elsewhere follows the file: deck A's position lands on deck B's track.
    let crossed = mapping(json!({ "deckA.tempo": { "module": "tempoB", "param": "Target BPM" } }));
    let b = resolve_bindings(&crossed, &two_decks()).bindings;
    assert_eq!(
        stretch_targets(&b, &tempos(), &[]),
        IndexMap::from([("deckB".to_owned(), tempos()["deckA"].clone())])
    );
}

#[test]
fn a_binding_the_engine_answers_for_is_not_stretched() {
    let b = vec![bound(None)];
    assert!(stretch_targets(&b, &tempos(), &["deckA:crossfade".to_owned()]).is_empty());
}

#[test]
fn a_value_goes_through_the_controls_range_then_the_widgets() {
    let linear = bound(Some(json!({ "min": -60, "max": 6 })));
    assert!((value_for(&linear, 0.5) + 27.0).abs() < 1e-9);
    // The widget's log curve puts the middle at the geometric mean.
    let log = bound(Some(json!({ "min": 20, "max": 20000, "curve": "log" })));
    assert!((value_for(&log, 0.5) - (20.0_f64 * 20000.0).sqrt()).abs() < 1e-6);
    // Its step quantises.
    assert_eq!(
        value_for(&bound(Some(json!({ "min": 0, "max": 10, "step": 1 }))), 0.44),
        4.0
    );
    // Without a range to map onto the number goes through as it came.
    assert_eq!(value_for(&bound(None), 0.42), 0.42);
    let tempo = DeskBinding {
        control: mix_controls().into_iter().find(|c| c.id == "deckA.tempo").unwrap(),
        ..bound(Some(json!({ "min": 0, "max": 100 })))
    };
    assert!((value_for(&tempo, 155.0) - 50.0).abs() < 1e-9);
}

#[test]
fn a_mirrored_line_inverts_inside_the_controls_own_range() {
    let m = mapping(json!({ "crossfade": { "module": "crossfade", "param": "Mix", "invert": true } }));
    let g = graph(json!([step("crossfade", Some("p"), None)]));
    let b = &resolve_bindings(&m, &g).bindings[0];
    assert_eq!(value_for(b, 0.25), 0.75);
}

#[test]
fn an_offline_tempo_module_reads_both_its_numbers_and_zero_when_the_deck_has_none() {
    let m = mapping(json!({ "deckA.tempo": { "module": "tempoA", "param": "Target BPM" } }));
    let b = resolve_bindings(&m, &two_decks()).bindings;
    let read = offline_publish(&b, &tempos(), "Source BPM");
    assert_eq!(
        (
            read[0].param.as_str(),
            read[0].value,
            read[1].param.as_str(),
            read[1].value
        ),
        ("Target BPM", 126.0, "Source BPM", 120.0)
    );
    assert_eq!(
        offline_publish(&b, &DeckTempos::new(), "Source BPM")
            .iter()
            .map(|p| p.value)
            .collect::<Vec<_>>(),
        [0.0, 0.0]
    );
    // A real plugin reports itself, and an offline module that is not a tempo is not the deck's tempo.
    assert!(offline_publish(&[bound(None)], &tempos(), "Source BPM").is_empty());
    let cut = mapping(json!({ "deckA.stemBass": { "module": "stemsA", "param": "Bass" } }));
    let g: BindGraph =
        serde_json::from_value(json!({ "chains": { "deckA": { "from": "deckA", "use": [{ "module": "stemsA" }] } } }))
            .unwrap();
    assert!(offline_publish(&resolve_bindings(&cut, &g).bindings, &tempos(), "Source BPM").is_empty());
}
