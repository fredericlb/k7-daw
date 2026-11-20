//! The frozen list's own rules: the account first, a bypassed step shown, params as the file holds them (1p-D10).

use k7_file_service::chains::frozen_chains;
use serde_json::json;

use crate::support::chains::World;

const COMP: &str = "plugin({ type: \"internal\", name: \"Compressor\" })\n";
const VOX: &str =
    "cold(true)\nplugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\n";

fn chain(steps: &str) -> String {
    format!("steps([{steps}, to([chain(\"@out\")])])\n")
}

#[test]
fn the_account_comes_first_then_the_cold_ones_and_a_bypassed_step_is_shown() {
    let live = chain(
        "id(\"a\").extends(\"@k7-core/audio/comp\").params({ Ratio: 2, Mode: \"soft\" }).bypass(true)",
    );
    let cold = chain("id(\"s\").extends(\"@k7-core/cold/vox\")");
    let w = World::new(
        "chains-frozen",
        &[
            ("libraries/k7-core/modules/audio/comp/module.k7x", COMP),
            ("libraries/k7-core/modules/cold/vox/module.k7x", VOX),
            ("libraries/aaa/chains/a-sep.k7x", &cold),
            ("libraries/aaa/chains/a-live.k7x", &live),
            ("libraries/k7-core/modules/cold/vox/assets/vox.onnx", "m"),
            ("studio/library/chains/mine.k7x", &live),
        ],
    );
    let got = frozen_chains(&w.chains()).expect("a list");
    let refs: Vec<&str> = got["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["ref"].as_str().unwrap())
        .collect();
    // The two halves are sorted apart: `@aaa/a-sep` sorts before `@account/mine` and comes after it, and the live `@aaa` one is out.
    assert_eq!(refs, ["@account/mine", "@aaa/a-sep"]);
    let step = &got["chains"][0]["steps"][0];
    assert_eq!(
        step,
        &json!({ "id": "a", "module": "@k7-core/audio/comp", "params": { "Ratio": 2, "Mode": "soft" }, "bypass": true })
    );
    assert!(
        got.to_string().contains("\"Ratio\":2,"),
        "an integer carries no fraction: {got}"
    );
    assert_eq!(got["chains"][1]["cold"], true);
    assert_eq!(got["chains"][0]["mine"], true);
}

#[test]
fn an_engines_params_come_before_the_modules_and_a_module_with_neither_has_none() {
    let both = chain(
        "id(\"d\").plugin({ type: \"internal\", name: \"Delay\" }).engine(\"tape\").engines({ tape: { params: { Wow: 0.2 } } }).params({ Wow: 1 })",
    );
    let bare = chain(
        "id(\"d\").plugin({ type: \"internal\", name: \"Delay\" }).engine(\"tape\").engines({ tape: {} }).params({ Wow: 1 })",
    );
    let none = chain("id(\"d\").plugin({ type: \"internal\", name: \"Delay\" })");
    let w = World::new(
        "chains-frozen-engine",
        &[
            ("studio/library/chains/a.k7x", &both),
            ("studio/library/chains/b.k7x", &bare),
            ("studio/library/chains/c.k7x", &none),
        ],
    );
    let got = frozen_chains(&w.chains()).expect("a list");
    let params = |i: usize| got["chains"][i]["steps"][0]["params"].clone();
    assert_eq!(
        (params(0), params(1), params(2)),
        (json!({ "Wow": 0.2 }), json!({ "Wow": 1 }), json!({}))
    );
    assert_eq!(got["chains"][0]["steps"][0]["module"], "Delay");
}

#[cfg(target_os = "linux")]
#[test]
fn two_names_that_differ_only_in_normalization_are_two_refs_in_code_point_order() {
    // Only a file system that keeps normalization apart holds both: Linux's does, a Mac's does not.
    let one = chain("id(\"a\").plugin({ type: \"internal\", name: \"X\" })");
    let (nfd, nfc) = (
        format!("studio/library/chains/e\u{301}.k7x"),
        "studio/library/chains/\u{e9}.k7x".to_owned(),
    );
    let w = World::new("chains-frozen-nfd", &[(&nfd, &one), (&nfc, &one)]);
    assert_eq!(w.chains().refs(), ["@account/e\u{301}", "@account/\u{e9}"]);
}

#[test]
fn a_chain_that_does_not_parse_is_listed_unreadable_refused_live_and_no_cold_chain() {
    use k7_file_service::chains::{
        ChainSettings, ColdRun, cold_chains, live_chains, resolve_cold_run,
    };
    let w = World::new(
        "chains-bad",
        &[
            ("libraries/k7-core/modules/audio/comp/module.k7x", COMP),
            ("studio/library/chains/bad.k7x", "steps(\n"),
        ],
    );
    let frozen = frozen_chains(&w.chains()).unwrap();
    let row = &frozen["chains"][0];
    assert_eq!(row["ref"], "@account/bad");
    assert_eq!((&row["cold"], &row["steps"]), (&json!(false), &json!([])));
    assert!(!row["unreadable"].as_str().unwrap().is_empty());
    let live = live_chains(&w.chains()).unwrap();
    assert!(
        live["chains"][0]["unavailable"]
            .as_str()
            .unwrap()
            .starts_with("it does not read — ")
    );
    assert_eq!(cold_chains(&w.chains()).unwrap(), json!([]));
    let run = resolve_cold_run(
        &w.chains(),
        &ChainSettings::default(),
        "stems",
        "@account/bad",
    );
    assert!(
        matches!(run, Ok(ColdRun::Refused { status: 400, .. })),
        "{run:?}"
    );
}
