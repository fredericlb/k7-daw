// ai_check_tests.rs — what a proposed panel must pass, case by case; the cases are the old TypeScript tests' inputs.

use super::*;
use serde_json::json;

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

const OK: &str = "cols(4)\nrows(2)\nwidgets({\n  cut: { type: \"knob\", col: 0, row: 0, colSpan: 2, rowSpan: 2, bind: \"Cutoff\", min: 0, max: 1 },\n})\n";

fn check(ui: &str, js: &str, binds: &[&str]) -> Vec<String> {
    check_ai_result(&proposal(ui, js), &names(binds), &AiContext::default()).issues
}

#[test]
fn a_clean_panel_passes() {
    assert_eq!(check(OK, "", &["Cutoff"]), Vec::<String>::new());
}

#[test]
fn a_bind_that_is_no_param_is_named() {
    let got = check(OK, "", &["Other"]);
    assert_eq!(got, vec!["`cut` binds `Cutoff`, which is not a parameter".to_owned()]);
}

#[test]
fn a_text_that_is_no_k7x_is_said_so() {
    let got = check("cols(", "", &[]);
    assert!(got[0].starts_with("the panel does not read as k7x: "), "{got:?}");
}

#[test]
fn a_panel_the_loader_refuses_is_refused_with_its_path() {
    let got = check(
        "cols(4)\nrows(2)\nwidgets({ a: { type: \"knob\", col: 0, row: 0, text: \"x\" } })\n",
        "",
        &[],
    );
    assert!(got[0].starts_with("panel widgets.a.text: "), "{got:?}");
}

#[test]
fn an_overlap_and_a_lone_group_are_named() {
    let ui = "cols(4)\nrows(2)\nwidgets({\n  a: { type: \"knob\", col: 0, row: 0, bind: \"A\" },\n  b: { type: \"knob\", col: 0, row: 0, bind: \"A\" },\n  g: { type: \"group\", col: 2, row: 0, colSpan: 2, rowSpan: 2 },\n})\n";
    let got = check(ui, "", &["A"]);
    assert!(got.contains(&"panel: `a` overlaps `b`".to_owned()), "{got:?}");
    assert!(
        got.contains(&"group `g` frames nothing: a group holds two or more — drop it".to_owned()),
        "{got:?}"
    );
}

#[test]
fn a_screen_needs_its_draw_function() {
    let ui = "cols(4)\nrows(2)\nwidgets({ scr: { type: \"lcdscreen\", col: 0, row: 0, colSpan: 2, rowSpan: 2 } })\n";
    assert_eq!(
        check(ui, "", &[]),
        vec!["lcdscreen `scr` has no draw function in widgets_js".to_owned()]
    );
    assert_eq!(check(ui, "export default { scr(lcd) {} }", &[]), Vec::<String>::new());
    assert_eq!(
        check(ui, "export default { 'scr': (l) => {} }", &[]),
        Vec::<String>::new()
    );
    assert!(!draws_for("export default { scrx(lcd) {} }", "scr"));
}

#[test]
fn a_dry_and_a_wet_level_blend_through_one_crossfade_knob() {
    let ui = "cols(4)\nrows(2)\nwidgets({\n  d: { type: \"knob\", col: 0, row: 0, bind: \"Dry\" },\n  w: { type: \"knob\", col: 2, row: 0, bind: \"Wet\" },\n})\n";
    let got = check(ui, "", &["Dry", "Wet"]);
    assert_eq!(
        got,
        vec![
            "`d` and `w` set `Dry` and `Wet` apart: blend them with one crossfade knob — `bind: Wet, pair: Dry`"
                .to_owned()
        ]
    );
}

#[test]
fn eq_bands_are_faders_and_at_params_stay_off() {
    let ui = "cols(4)\nrows(2)\nwidgets({\n  l: { type: \"knob\", col: 0, row: 0, bind: \"Low Shelf Gain\" },\n  m: { type: \"knob\", col: 2, row: 0, bind: \"@pan\" },\n})\n";
    let got = check(ui, "", &["Low Shelf Gain", "@pan"]);
    assert!(got.iter().any(|i| i.contains("an EQ band: make it a fader")), "{got:?}");
    assert!(
        got.iter().any(|i| i.contains("leave `@` params off the panel")),
        "{got:?}"
    );
}

#[test]
fn a_pointer_nobody_asked_for_is_refused() {
    let ui = "cols(4)\nrows(2)\nwidgets({ s: { type: \"lcdscreen\", col: 0, row: 0, colSpan: 2, rowSpan: 2 } })\n";
    let js = "export default { s: { draw(l) {}, pointer(l, e) {} } }";
    let asked = |d: &str, mode| AiContext {
        direction: d.into(),
        widgets_js: String::new(),
        mode,
    };
    let refused = check_ai_result(&proposal(ui, js), &[], &asked("", AiMode::Reset));
    assert_eq!(refused.issues.len(), 1);
    assert!(
        check_ai_result(&proposal(ui, js), &[], &asked("make it draggable", AiMode::Reset))
            .issues
            .is_empty()
    );
    assert!(
        check_ai_result(&proposal(ui, js), &[], &asked("fais glisser", AiMode::Reset))
            .issues
            .is_empty()
    );
}

#[test]
fn a_request_is_checked_before_a_minute_of_claude_is_spent() {
    let body = json!({ "module": "m", "moduleText": "", "params": [], "uiText": "", "widgetsJs": "", "mode": "update", "direction": "  " });
    assert_eq!(
        parse_request(&body).unwrap_err(),
        "direction: an update says what should change"
    );
    let mut ok = body.clone();
    ok["direction"] = json!("more reverb");
    assert!(parse_request(&ok).is_ok());
    ok["extra"] = json!(1);
    assert!(parse_request(&ok).is_ok());
    ok["module"] = json!("");
    assert!(parse_request(&ok).is_err());
}

#[test]
fn the_user_prompt_shows_the_panel_only_on_an_update() {
    let mut req = parse_request(&json!({
        "module": "verb", "moduleText": "plugin({ type: \"internal\" })", "uiText": "cols(2)", "widgetsJs": "",
        "params": [{ "name": "Size", "meta": { "min": 0, "max": 1, "def": 0.5 }, "value": "0.3" }],
        "mode": "reset", "direction": "",
    }))
    .unwrap();
    let reset = ai_user_prompt(&req);
    assert!(
        reset.contains("- \"Size\": range 0..1, default 0.5, value 0.3"),
        "{reset}"
    );
    assert!(reset.contains("## Design a new panel"));
    assert!(!reset.contains("Current ui.k7x"));
    req.mode = AiMode::Update;
    req.direction = "bigger".into();
    let update = ai_user_prompt(&req);
    assert!(update.contains("## Current ui.k7x\n```k7x\ncols(2)\n```"), "{update}");
    assert!(update.ends_with("## What to change\nbigger"));
}
