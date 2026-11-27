// auto_tests.rs — which family a name suggests and the panel proposed from them; the cases are the old TypeScript tests' inputs.

use super::*;

fn p(name: &str) -> AutoParam {
    AutoParam {
        name: name.to_owned(),
        meta: None,
    }
}

fn pages(params: &[&str]) -> Vec<String> {
    let ps: Vec<AutoParam> = params.iter().map(|n| p(n)).collect();
    group_params(&ps).into_iter().map(|g| g.page).collect()
}

#[test]
fn a_word_two_families_want_goes_where_the_tables_order_says() {
    for (name, page) in [
        ("Comp Make Up gain", "Dynamics"),
        ("Comp Attack", "Dynamics"),
        ("Low Shelf Gain", "EQ"),
        ("Bell Freq", "EQ"),
        ("Reverb Decay Start", "Reverb"),
        ("Reverb Dry/Wet", "Reverb"),
        ("MPE_Brightness_0", "MIDI"),
        ("Navigate through filters", "Preset"),
        ("Active Filter Category", "Preset"),
        ("Pitch Transpose", "Tuning"),
        ("Pitch Bend", "MIDI"),
        ("String Tension", "Other"),
        ("Master Volume", "Mix"),
        ("@base", GENERAL_PAGE),
    ] {
        assert_eq!(family_of(name), page, "{name}");
    }
}

#[test]
fn plumbing_is_left_out_and_the_blend_is_the_modules_own() {
    assert!(is_plumbing("Select Preset") && is_plumbing("@pan") && is_plumbing("MPE_PitchBend_3"));
    assert!(!is_plumbing("@base") && !is_plumbing("Filter Cutoff"));
    assert!(is_blend("Mix") && is_blend("Dry/Wet") && is_blend("@mix"));
    assert!(!is_blend("Reverb Dry/Wet") && !is_blend("Delay Mix") && !is_blend("Master Volume"));
    assert_eq!(family_of("@base_mix"), "Track");
    assert_eq!(
        pages(&["String Tension", "Master Volume", "Reverb Room Size"]),
        ["Reverb", "Mix", "Other"]
    );
    assert_eq!(
        pages(&["Select Preset", "VST_ProgramChange_7", "Filter Cutoff"]),
        ["Filter"]
    );
}

#[test]
fn a_panel_is_the_families_as_tabs_with_each_params_own_range() {
    let with = |name: &str, min: f64, max: f64| AutoParam {
        name: name.to_owned(),
        meta: Some(AutoMeta {
            min: Some(min),
            max: Some(max),
            ..AutoMeta::default()
        }),
    };
    let ui = auto_organize(&[with("Filter Cutoff", 20.0, 20000.0), with("Master Volume", -60.0, 6.0)]);
    assert_eq!(ui["widgets"], json!({}));
    let names: Vec<&str> = ui["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names, ["Filter", "Mix"]);
    assert_eq!(ui["tabs"][0]["widgets"]["w1"]["max"], 20000.0);
    assert_eq!(
        auto_organize(&[p("Select Preset")]),
        json!({ "cols": 2, "rows": 1, "widgets": {} })
    );
    // The engine's stage replaces the plugin's own blend and is the footer, never a control.
    let staged = auto_organize(&[p("@mix"), p("Wet"), p("Filter Cutoff")]);
    assert_eq!(staged["widgets"].as_object().map(|m| m.len()), Some(1));
}
