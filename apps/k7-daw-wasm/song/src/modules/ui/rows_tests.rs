// rows_tests.rs — the params a module's widgets may bind, unioned from every source that knows some: the cases the builder's pane relies on.

use super::*;
use crate::doc::{INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM};

const MODULE: &str = "// a delay\nplugin({ type: \"internal\", name: \"delay\" })\nparams({ Time: 0.5, \"Wet Level\": -3.0, Mode: \"ping\" })\n";

#[test]
fn the_footer_is_an_effects_three_an_instruments_one_and_nothing_for_an_io_module() {
    assert_eq!(
        stage_for("modules/audio/d/module.k7x", None),
        [INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM]
    );
    assert_eq!(
        stage_for("_libraries/x/_modules/instruments/s/module.k7x", None),
        [OUTPUT_VOLUME_PARAM]
    );
    assert!(stage_for("modules/audio/d/module.k7x", Some("in")).is_empty());
    assert!(stage_for("modules/audiox/d/module.k7x", None).is_empty());
}

#[test]
fn the_files_own_params_read_as_the_text_they_are_written_in_and_as_numbers_where_they_are_one() {
    let read = read_params(MODULE);
    assert_eq!(
        Value::Object(read),
        json!({"Time": "0.5", "Wet Level": "-3.0", "Mode": "ping"})
    );
    assert_eq!(Value::Object(param_values(MODULE)), json!({"Time": 0.5, "Wet Level": -3}));
}

fn row<'a>(rows: &'a [ParamRowOut], name: &str) -> Option<&'a ParamRowOut> {
    rows.iter().find(|r| r.name == name)
}

#[test]
fn the_rows_union_every_source_once_the_most_specific_range_winning_the_engines_own_binds_on_an_effect() {
    let rows = param_rows(&RowSources {
        content: Some(MODULE.into()),
        bound: vec!["Time".into()],
        path: "modules/audio/d/module.k7x".into(),
        hint_params: vec!["Time".into(), "Wobble".into()],
        hint_meta: json!({"Wobble": {"min": 0, "max": 1}, "Time": {"min": 0, "max": 9}})
            .as_object()
            .cloned()
            .unwrap(),
        plugin_names: vec!["Wobble".into()],
        plugin_meta: json!({"Time": {"min": 0, "max": 2}}).as_object().cloned().unwrap(),
        ..RowSources::default()
    });
    assert_eq!(rows.iter().filter(|r| r.name == "Time").count(), 1);
    let time = row(&rows, "Time").unwrap();
    assert!(time.bound);
    assert_eq!(time.value.as_deref(), Some("0.5"));
    assert_eq!(time.meta, Some(json!({"min": 0, "max": 2})));
    assert_eq!(row(&rows, "Wobble").unwrap().meta, Some(json!({"min": 0, "max": 1})));
    // The footer an effect has, and a lane's intrinsics, are offered too.
    for n in ["@mix", "@input-volume", "@output-volume", "@pan", "@mute"] {
        assert!(row(&rows, n).is_some(), "{n}");
    }
    assert_eq!(row(&rows, "@mix").unwrap().meta, Some(json!({"min": 0, "max": 1, "def": 1})));
    // The built-in the file names knows its own params without any engine.
    assert!(rows.len() > 8);
}

#[test]
fn an_io_module_is_offered_its_own_settings_and_one_the_file_states_at_its_top_level_is_read() {
    let rows = param_rows(&RowSources {
        content: Some("io(\"in\")\nmonitor: auto\n".into()),
        io: Some("in".into()),
        path: "modules/io/in/module.k7x".into(),
        ..RowSources::default()
    });
    assert_eq!(row(&rows, "@monitor").unwrap().value.as_deref(), Some("auto"));
    assert_eq!(row(&rows, "@gain").unwrap().meta, Some(json!({"min": -24, "max": 24})));
    assert!(row(&rows, "@mix").is_none());
}
