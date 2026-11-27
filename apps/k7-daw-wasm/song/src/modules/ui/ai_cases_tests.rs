// ai_cases_tests.rs — what AI organize tells the model and what it refuses from it, each case on a whole panel as a proposal carries it.
// The model is not under test: the CHECK is. Every way a proposal can break the panel must come back as a sentence the model can fix.

use super::*;
use crate::modules::ui::ai_answer::read_ai_answer;
use crate::modules::ui::ai_prompt::ai_design_brief;
use serde_json::json;

const PARAMS: [&str; 4] = ["Cutoff", "Resonance", "Attack", "Mode"];

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn proposal(ui: &str, js: &str) -> AiResult {
    AiResult {
        ui_text: ui.to_owned(),
        widgets_js: js.to_owned(),
        ..AiResult::default()
    }
}

/// The check MINUS its two-layout findings: these fixtures are about binds, screens, frames and racks.
fn checked(ui: &str, js: &str, binds: &[&str], ctx: &AiContext) -> Checked {
    let mut c = check_ai_result(&proposal(ui, js), &names(binds), ctx);
    let shape: Vec<String> = c.ui.as_ref().map(twin_issues).unwrap_or_default();
    c.issues.retain(|i| !shape.contains(i));
    c
}

fn other(ui: &str, js: &str) -> Vec<String> {
    checked(ui, js, &PARAMS, &AiContext::default()).issues
}

const GOOD_WIDGETS: &str = "  screen: { type: \"lcdscreen\", col: 0, row: 0, colSpan: 8, rowSpan: 2 },\n  cutoff: { type: \"knob\", col: 0, row: 2, colSpan: 2, rowSpan: 2, label: \"Cutoff\", bind: \"Cutoff\", min: 20, max: 20000, curve: \"log\" },\n  res: { type: \"knob\", col: 2, row: 2, colSpan: 2, rowSpan: 2, label: \"Res\", bind: \"Resonance\", min: 0, max: 1 },\n";

/// The filter's panel, with more widget lines added to its map.
fn good_ui(more: &str) -> String {
    format!("// filter\ncols(8)\nrows(5)\nwidgets({{\n{GOOD_WIDGETS}{more}}})\n")
}

const GOOD_JS: &str = "export default {\n  screen(lcd) {\n    lcd.ctx.fillStyle = \"#000\";\n    lcd.ctx.fillRect(0, 0, lcd.width, lcd.height);\n  },\n};\n";

#[test]
fn the_prompt_quotes_every_param_name_exactly_and_the_authors_direction_trimmed() {
    let req = parse_request(&json!({
        "module": "filter", "moduleText": "extends(\"@engine/lowpass\")",
        "params": [
            { "name": "Cutoff", "meta": { "min": 20, "max": 20000 }, "value": "440" },
            { "name": "Mode", "meta": { "min": 0, "max": 1, "states": [{ "label": "LP", "value": 0 }] } },
        ],
        "uiText": "", "widgetsJs": "", "mode": "reset", "direction": "  make the screen big ",
    }))
    .unwrap();
    let text = ai_user_prompt(&req);
    assert!(text.contains("- \"Cutoff\": range 20..20000, value 440"), "{text}");
    assert!(text.contains("\"Mode\": states: [{\"label\":\"LP\",\"value\":0}]"), "{text}");
    assert!(text.contains("## What the author asks for\nmake the screen big"), "{text}");
}

#[test]
fn the_brief_describes_every_widget_kind_the_schema_accepts() {
    let brief = ai_design_brief();
    for t in crate::modules::WIDGET_TYPES {
        assert!(brief.contains(&format!("`{t}`")), "{t}");
    }
}

fn answer(panel: Value, extra: Value) -> String {
    let mut a = json!({ "header": "filter — a low pass", "panel": panel, "widgets_js": "```js\nexport default {};\n```", "notes": "n" });
    for (k, v) in extra.as_object().into_iter().flatten() {
        a[k] = v.clone();
    }
    a.to_string()
}

#[test]
fn an_answer_is_printed_as_ui_k7x_a_widget_a_line_and_the_script_loses_its_fence() {
    let r = read_ai_answer(&answer(
        json!({ "cols": 4, "rows": 2, "widgets": [
            { "id": "cut", "type": "knob", "col": 0, "row": 0, "bind": "Cutoff" },
            { "id": "res", "type": "knob", "col": 2, "row": 0, "bind": "Resonance" },
        ], "tabs": [] }),
        json!({}),
    ))
    .unwrap();
    assert_eq!(
        r.ui_text,
        "// filter — a low pass\ncols(4)\nrows(2)\nwidgets({\n  cut: { type: \"knob\", col: 0, row: 0, bind: \"Cutoff\" },\n  res: { type: \"knob\", col: 2, row: 0, bind: \"Resonance\" },\n})\n"
    );
    assert_eq!((r.widgets_js.as_str(), r.notes.as_str()), ("export default {};\n", "n"));
}

#[test]
fn the_tabs_and_the_vertical_twins_lists_become_maps_too() {
    let w = json!({ "id": "cut", "type": "knob", "col": 0, "row": 0, "bind": "Cutoff" });
    let r = read_ai_answer(&answer(
        json!({ "cols": 8, "rows": 2, "widgets": [],
            "tabs": [{ "name": "Main", "widgets": [w] }],
            "vertical": { "cols": 4, "rows": 2, "widgets": [w], "tabs": [{ "name": "Main", "widgets": [] }] } }),
        json!({}),
    ))
    .unwrap();
    let ui = check_ai_result(&r, &names(&["Cutoff"]), &AiContext::default()).ui.unwrap();
    assert_eq!(ui["tabs"][0]["widgets"]["cut"]["bind"], json!("Cutoff"));
    assert_eq!(ui["vertical"]["widgets"]["cut"]["type"], json!("knob"));
}

#[test]
fn an_id_used_twice_in_one_list_is_reported_rather_than_losing_a_widget_silently() {
    let w = json!({ "id": "cut", "type": "knob", "col": 0, "row": 0, "bind": "Cutoff" });
    let mut again = w.clone();
    again["col"] = json!(2);
    let r = read_ai_answer(&answer(json!({ "cols": 4, "rows": 2, "widgets": [w, again] }), json!({}))).unwrap();
    assert_eq!(r.issues, ["panel: widget id `cut` is used twice"]);
    let got = check_ai_result(&r, &names(&["Cutoff"]), &AiContext::default());
    assert!(got.issues.contains(&"panel: widget id `cut` is used twice".to_owned()));
}

#[test]
fn an_answer_that_is_not_the_promised_shape_is_none() {
    assert!(read_ai_answer("not json").is_none());
    assert!(read_ai_answer(&json!({ "panel": 3 }).to_string()).is_none());
    assert!(read_ai_answer(&answer(json!({ "cols": 1 }), json!({ "widgets_js": 3 }))).is_none());
}

#[test]
fn a_sound_panel_with_a_drawn_screen_passes_keeping_its_widget_order() {
    let c = checked(&good_ui(""), GOOD_JS, &PARAMS, &AiContext::default());
    assert!(c.issues.is_empty(), "{:?}", c.issues);
    let order: Vec<&str> = c.ui.as_ref().unwrap()["widgets"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(order, ["screen", "cutoff", "res"]);
}

const TOUCH: &str = "export default { screen: { draw(lcd) {}, pointer(lcd, ev) { return true; } } };";

fn asked(direction: &str, mode: AiMode, widgets_js: &str) -> AiContext {
    AiContext {
        direction: direction.into(),
        widgets_js: widgets_js.into(),
        mode,
    }
}

fn with(ui: &str, js: &str, ctx: &AiContext) -> Vec<String> {
    checked(ui, js, &PARAMS, ctx).issues
}

#[test]
fn an_interactive_screen_nobody_asked_for_is_refused_and_one_that_was_asked_for_taken() {
    let ui = good_ui("");
    let refused = ["widgets_js gives a screen a `pointer` handler, and the author did not ask for an interactive screen: make every screen a plain draw function"];
    assert_eq!(with(&ui, TOUCH, &asked("tidy it", AiMode::Reset, "")), refused);
    assert_eq!(other(&ui, TOUCH).len(), 1);
    assert!(with(&ui, TOUCH, &asked("let me drag the cutoff on the screen", AiMode::Reset, "")).is_empty());
    // An update keeps an interactive screen that already was one.
    assert!(with(&ui, TOUCH, &asked("bigger knobs", AiMode::Update, TOUCH)).is_empty());
}

#[test]
fn a_script_that_does_not_compile_is_named() {
    let broken = other(&good_ui(""), &format!("{GOOD_JS}{{"));
    assert!(broken[0].starts_with("widgets_js does not compile"), "{broken:?}");
}

#[test]
fn overlaps_and_widgets_outside_the_grid_are_named() {
    let bad = good_ui("").replace("col: 2, row: 2", "col: 1, row: 2").replace("rows(5)", "rows(3)");
    let issues = other(&bad, GOOD_JS);
    assert!(issues.contains(&"panel: `cutoff` runs outside the 8×3 grid".to_owned()), "{issues:?}");
    assert!(issues.contains(&"panel: `cutoff` overlaps `res`".to_owned()), "{issues:?}");
}

#[test]
fn each_tab_is_checked_together_with_the_panel_level_widgets_drawn_over_it() {
    let tabs = "cols(8)\nrows(6)\nwidgets({\n  screen: { type: \"lcdscreen\", col: 0, row: 4, colSpan: 8, rowSpan: 2 },\n})\ntabs([\n  name(\"Main\").widgets({\n    cutoff: { type: \"knob\", col: 0, row: 0, colSpan: 2, rowSpan: 2, bind: \"Cutoff\", min: 0, max: 1 },\n  }),\n  name(\"Env\").widgets({\n    attack: { type: \"knob\", col: 0, row: 3, colSpan: 2, rowSpan: 2, bind: \"Attack\", min: 0, max: 1 },\n    cutoff: { type: \"knob\", col: 2, row: 0, bind: \"Cutoff\", min: 0, max: 1 },\n  }),\n])\n";
    let issues = other(tabs, GOOD_JS);
    assert!(issues.contains(&"tab Env: `screen` overlaps `attack`".to_owned()), "{issues:?}");
    assert!(issues.contains(&"widget id `cutoff` is used twice".to_owned()), "{issues:?}");
    assert!(!issues.iter().any(|i| i.starts_with("tab Main")), "{issues:?}");
}

#[test]
fn a_group_frames_the_widgets_wholly_inside_it_and_a_straddling_one_is_named() {
    let grouped = good_ui("  filt: { type: \"group\", col: 0, row: 2, colSpan: 4, rowSpan: 2, label: \"FILTER\", color: \"#3d7dff26\" },\n");
    assert!(other(&grouped, GOOD_JS).is_empty());
    let straddling = grouped.replace("colSpan: 4, rowSpan: 2, label: \"FILTER\"", "colSpan: 3, rowSpan: 2, label: \"FILTER\"");
    assert!(other(&straddling, GOOD_JS).contains(
        &"panel: `res` and `filt` cross — a widget lies wholly inside a group or wholly outside it, and groups never overlap".to_owned()
    ));
}

#[test]
fn a_crossfade_knob_takes_a_dry_and_a_wet_param_and_a_pair_that_is_not_a_parameter_is_named() {
    let xf = |pair: &str| good_ui(&format!("  mix: {{ type: \"knob\", col: 4, row: 2, label: \"MIX\", bind: \"Attack\", pair: \"{pair}\" }},\n"));
    assert!(other(&xf("Mode"), GOOD_JS).is_empty());
    assert_eq!(other(&xf("Dry"), GOOD_JS), ["`mix` pairs `Dry`, which is not a parameter"]);
}

#[test]
fn a_pair_anywhere_but_on_a_knob_and_one_equal_to_its_bind_are_refused() {
    let slider = good_ui("  s: { type: \"slider\", col: 4, row: 2, bind: \"Attack\", pair: \"Mode\" },\n");
    assert!(other(&slider, GOOD_JS)[0].starts_with("panel widgets.s.pair"));
    let same = good_ui("  k: { type: \"knob\", col: 4, row: 2, bind: \"Attack\", pair: \"Attack\" },\n");
    assert!(other(&same, GOOD_JS)[0].starts_with("panel widgets.k.pair"));
}

#[test]
fn a_colour_anywhere_but_on_a_group_is_refused_and_a_groups_on_off_bind_taken() {
    let bad = good_ui("").replace("label: \"Res\",", "label: \"Res\", color: \"#fff\",");
    assert!(other(&bad, GOOD_JS)[0].starts_with("panel widgets.res.color"));
    let bound = good_ui("  g: { type: \"group\", col: 4, row: 2, bind: \"Cutoff\" },\n");
    assert!(!other(&bound, GOOD_JS).iter().any(|i| i.starts_with("panel widgets.g.bind")));
}

#[test]
fn the_schemas_own_complaint_is_handed_back_with_its_path() {
    let bad = good_ui("").replace("label: \"Res\",", "label: \"Res\", options: [],");
    let c = checked(&bad, GOOD_JS, &PARAMS, &AiContext::default());
    assert!(c.ui.is_none());
    assert!(c.issues[0].starts_with("panel widgets.res"), "{:?}", c.issues);
}

const KIT: &str = "type(\"rack\")\nrack({ symbols: { BD: { note: 36 }, SN: { note: 38 } } })\n";
const KIT_UI: &str = "cols(9)\nrows(4)\nwidgets({\n  bd: { type: \"tap\", col: 0, row: 0, colSpan: 3, rowSpan: 2, bind: \"@rack:BD\" },\n  sn: { type: \"tap\", col: 3, row: 0, colSpan: 3, rowSpan: 2, bind: \"@rack:SN\" },\n  add: { type: \"tap\", col: 6, row: 0, colSpan: 3, rowSpan: 2, bind: \"@rack+\" },\n  vol: { type: \"knob\", col: 0, row: 2, colSpan: 2, rowSpan: 2, bind: \"Volume\", min: 0, max: 1 },\n})\n";

fn volume() -> Vec<AiParam> {
    vec![AiParam {
        name: "Volume".into(),
        meta: None,
        value: None,
    }]
}

fn rack_check(ui: &str, binds: &[String]) -> Vec<String> {
    let refs: Vec<&str> = binds.iter().map(String::as_str).collect();
    checked(ui, "", &refs, &AiContext::default()).issues
}

#[test]
fn a_rack_module_may_bind_the_sounds_its_rack_declares_and_the_add_tap() {
    let binds = bindable_names(&volume(), KIT);
    assert_eq!(binds, ["Volume", "@rack:BD", "@rack:SN", "@rack+"]);
    assert!(rack_check(KIT_UI, &binds).is_empty());
}

#[test]
fn a_tap_on_a_sound_the_rack_does_not_have_is_named() {
    let binds = bindable_names(&volume(), KIT);
    let bad = KIT_UI.replace("\"@rack:SN\"", "\"@rack:HH\"");
    assert_eq!(rack_check(&bad, &binds), ["tap `sn` binds `@rack:HH`, which is not a sound of this module's rack"]);
}

#[test]
fn pads_stay_on_taps_and_taps_on_pads() {
    let binds = bindable_names(&volume(), KIT);
    let knob_on_pad = KIT_UI.replace("bind: \"Volume\"", "bind: \"@rack:BD\"");
    let tap_on_param = KIT_UI.replace("\"@rack:SN\"", "\"Volume\"");
    assert_eq!(rack_check(&knob_on_pad, &binds), ["`vol` binds a rack sound, which only a tap may"]);
    assert_eq!(rack_check(&tap_on_param, &binds), ["tap `sn` must bind a rack sound or `@rack+`"]);
}

#[test]
fn a_module_with_no_rack_is_offered_no_tap_at_all() {
    let plain = bindable_names(&volume(), "type(\"fx\")\n");
    assert_eq!(plain, ["Volume"]);
    assert_eq!(
        rack_check(KIT_UI, &plain)[0],
        "tap `bd` binds `@rack:BD`, which is not a sound of this module's rack"
    );
}

fn request(mode: &str, module_text: &str, direction: &str) -> AiRequest {
    parse_request(&json!({
        "module": "filter", "moduleText": module_text, "params": [{ "name": "Cutoff" }],
        "uiText": good_ui(""), "widgetsJs": GOOD_JS, "mode": mode, "direction": direction,
    }))
    .unwrap()
}

#[test]
fn the_racks_sounds_are_listed_in_the_prompt() {
    let prompt = ai_user_prompt(&request("reset", KIT, ""));
    assert!(prompt.contains("## Rack sounds (2)"));
    assert!(prompt.contains("- \"@rack:SN\""));
}

#[test]
fn a_panel_that_lays_something_out_is_told_from_one_that_does_not() {
    assert!(has_layout(&good_ui("")));
    assert!(!has_layout("cols(8)\nrows(5)\nwidgets({})\n"));
    assert!(!has_layout(""));
    assert!(!has_layout("widgets([unclosed"));
}

#[test]
fn an_update_sends_the_panel_there_is_and_what_to_change() {
    let text = ai_user_prompt(&request("update", "", "make the cutoff bigger"));
    assert!(text.contains("## Current ui.k7x"));
    assert!(text.contains("cutoff: { type: \"knob\""));
    assert!(text.contains("## Current widgets.js"));
    assert!(text.contains("## What to change\nmake the cutoff bigger"));
    assert!(text.contains("keep everything else as it is"));
}

#[test]
fn a_reset_sends_no_panel_at_all_so_none_of_it_can_come_back() {
    let text = ai_user_prompt(&request("reset", "", ""));
    assert!(!text.contains("## Current ui.k7x"));
    assert!(!text.contains("cutoff: { type: \"knob\""));
    assert!(!text.contains("widgets.js"));
    assert!(text.contains("## Design a new panel"));
    assert!(!text.contains("## What the author asks for"));
}

#[test]
fn a_group_around_one_widget_is_checked_inside_a_tab_as_well() {
    let tabs = "cols(8)\nrows(6)\nwidgets({})\ntabs([\n  name(\"Main\").widgets({\n    g: { type: \"group\", col: 0, row: 0, colSpan: 2, rowSpan: 2 },\n    cutoff: { type: \"knob\", col: 0, row: 0, colSpan: 2, rowSpan: 2, bind: \"Cutoff\", min: 0, max: 1 },\n  }),\n  name(\"Env\").widgets({}),\n])\n";
    assert!(other(tabs, GOOD_JS).contains(
        &"group `g` frames a single widget: a group holds two or more — drop it and leave the widget bare".to_owned()
    ));
}

#[test]
fn the_engines_blend_is_what_the_check_applies_wherever_mix_is_offered() {
    let ui = "cols(4)\nrows(2)\nwidgets({\n  d: { type: \"knob\", col: 0, row: 0, bind: \"Dry Level\" },\n  w: { type: \"knob\", col: 1, row: 0, bind: \"Wet Level\" },\n})\n";
    let issues = checked(ui, "", &["Dry Level", "Wet Level", "@mix"], &AiContext::default()).issues;
    assert!(issues.iter().any(|i| i.contains("the blend is the engine's")), "{issues:?}");
    assert!(!issues.iter().any(|i| i.contains("crossfade knob — `bind")), "{issues:?}");
}

const KNOBS: &str = "    cutoff: { type: \"knob\", col: 0, row: 0, colSpan: 2, rowSpan: 2, label: \"Cutoff\", bind: \"Cutoff\", min: 20, max: 20000, curve: \"log\" },\n    res: { type: \"knob\", col: 2, row: 0, colSpan: 2, rowSpan: 2, label: \"Res\", bind: \"Resonance\", min: 0, max: 1 },\n";

fn wide() -> String {
    let dedented: String = KNOBS.lines().map(|l| format!("{}\n", &l[2..])).collect();
    format!("cols(8)\nrows(2)\nwidgets({{\n{dedented}}})\n")
}

#[test]
fn a_panel_wider_than_four_columns_is_asked_for_a_vertical_layout() {
    let c = check_ai_result(&proposal(&wide(), ""), &names(&PARAMS), &AiContext::default());
    assert!(c.issues.join(" ").contains("no `vertical` layout"), "{:?}", c.issues);
}

#[test]
fn a_vertical_layout_offering_the_same_controls_is_taken_and_one_that_drops_a_control_named() {
    let twin = format!("{}vertical({{\n  cols: 4,\n  rows: 4,\n  widgets: {{\n{KNOBS}  }},\n}})\n", wide());
    let ctx = AiContext::default();
    assert_eq!(check_ai_result(&proposal(&twin, ""), &names(&PARAMS), &ctx).issues, Vec::<String>::new());
    let res_line = twin.rfind("    res: ").unwrap();
    let end = twin[res_line..].find('\n').unwrap() + res_line + 1;
    let short = format!("{}{}", &twin[..res_line], &twin[end..]);
    let c = check_ai_result(&proposal(&short, ""), &names(&PARAMS), &ctx);
    assert!(c.issues.join(" ").contains("does not offer the same controls"), "{:?}", c.issues);
}
