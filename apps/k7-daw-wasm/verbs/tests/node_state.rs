// node_state.rs — bypass, alternatives, unfreezing and the external UI on real fixtures: the files, the doc, undo and the refusals.
// A reader would get wrong a bypass's push: a node with a plugin is pushed (`applyProject`), the doc's `enabled` is what plays.

mod support;

use song_edit::Fs;
use support::Project;

fn put(p: &mut Project, abs: &str, text: &str) {
    let dir = abs.rsplit_once('/').map_or("", |(d, _)| d);
    p.fs.mkdir_all(dir).expect("a folder");
    p.fs.write_atomic(abs, text.as_bytes()).expect("a write");
}

fn enabled(p: &mut Project, lane: &str) -> Vec<bool> {
    p.track(lane).plugins.iter().map(|pl| pl.enabled).collect()
}

#[test]
fn a_bypass_toggles_the_node_and_pushes_the_mix() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("node bypass lead");
    assert_eq!(ran.answer.log, ["melody: lead bypassed"]);
    assert_eq!(ran.answer.effects, ["applyProject"]);
    assert_eq!(ran.changed(&p.dir), ["graph.k7x"]);
    p.ok("node bypass lead");
    assert_eq!(enabled(&mut p, "melody"), [false]);
    let ran = p.ok("node bypass lead");
    assert_eq!(ran.answer.log, ["melody: lead active"]);
    assert_eq!(enabled(&mut p, "melody"), [true]);
    let ran = p.ok("node bypass lead off");
    assert_eq!(ran.answer.log, ["melody: lead active"]);
    p.refused("node bypass ghost", "no module \"ghost\" in any chain");
    p.refused("node bypass lead maybe", "usage: node bypass");
}

#[test]
fn a_bypass_in_a_chain_project_is_the_step_item_s() {
    let mut p = Project::open_crate("chain-lanes");
    let comp = "00000000-0000-4000-8000-0000000000d2";
    let ran = p.round_trip(&format!("node bypass {comp} on"));
    assert_eq!(ran.changed(&p.dir), ["chains/drums.k7x"]);
    p.ok(&format!("node bypass {comp} on"));
    assert!(p.text("chains/drums.k7x").contains(".bypass(true)"));
    assert_eq!(enabled(&mut p, "drums"), [false]);
}

#[test]
fn an_alternative_is_created_copied_and_selected() {
    let mut p = Project::open("note-lane");
    p.ok("node param lead Level -6");
    let ran = p.round_trip("node alternative copy lead bright");
    assert_eq!(
        ran.answer.log,
        ["lead: alternative 'bright' — a copy of the one that was on"]
    );
    assert_eq!(ran.changed(&p.dir), ["nodes/lead.k7x"]);
    p.ok("node alternative copy lead bright");
    p.ok("node param lead Level -12");
    let ran = p.ok("node alternative select lead");
    assert!(ran.answer.log[0].contains("[bright]"), "{:?}", ran.answer.log);
    let first = ran.answer.log[0].split_whitespace().nth(1).unwrap_or("").to_owned();
    let ran = p.round_trip(&format!("node alternative select lead {first}"));
    assert_eq!(ran.answer.log, [format!("lead: running alternative '{first}'")]);
    assert_eq!(
        ran.answer.effects,
        ["applyProject"],
        "two sets of numbers are a values push"
    );
    p.ok(&format!("node alternative select lead {first}"));
    assert_eq!(p.track("melody").plugins[0].params.get("Level"), Some(&-6.0));
    p.ok("node alternative select lead bright");
    assert_eq!(p.track("melody").plugins[0].params.get("Level"), Some(&-12.0));
    let ran = p.round_trip("node alternative create lead");
    assert!(
        ran.answer.log[0].ends_with("— empty, and running"),
        "{:?}",
        ran.answer.log
    );
    p.refused("node alternative select lead ghost", "node alternative:");
    p.refused("node alternative copy lead bright", "node alternative copy:");
}

#[test]
fn an_unfreeze_deletes_its_render() {
    let mut p = Project::open("note-lane");
    p.refused("node freeze ghost", "freeze: no module \"ghost\" in the chains");
    let lead = format!("{}/nodes/lead.k7x", p.dir);
    let text = p.text("nodes/lead.k7x");
    put(&mut p, &lead, &format!("{text}frozen(\"renders/lead.wav\")\n"));
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/chain-lanes.k7/clips/kick.wav"
    ))
    .expect("a wav");
    p.fs.mkdir_all(&format!("{}/renders", p.dir)).expect("a folder");
    p.fs.write_atomic(&format!("{}/renders/lead.wav", p.dir), &bytes)
        .expect("a write");
    let ran = p.round_trip("node freeze lead");
    assert_eq!(ran.answer.log, ["lead: unfrozen — everything upstream is live again"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/lead.k7x", "renders/lead.wav"]);
    p.ok("node freeze lead");
    assert!(!p.exists("renders/lead.wav"));
    assert!(!p.text("nodes/lead.k7x").contains("frozen"));
}

#[test]
fn an_external_ui_is_asked_of_the_host() {
    let mut p = Project::open("note-lane");
    p.refused("node external-ui", "no external plugins in this project");
    put(
        &mut p,
        "/w/ws/_modules/instruments/ext/module.k7x",
        "plugin({ type: \"vst3\", name: \"Ext Synth\" })\n",
    );
    let lead = format!("{}/nodes/lead.k7x", p.dir);
    put(&mut p, &lead, "extends(\"@workspace/instruments/ext\")\n");
    let doc = p.doc();
    let kinds: Vec<String> = doc
        .tracks
        .iter()
        .flat_map(|t| t.plugins.iter().map(|pl| pl.plugin_type.clone()))
        .collect();
    assert!(kinds.iter().any(|k| k == "vst3"), "{kinds:?}");
    let ran = p.ok("node external-ui LEAD");
    assert_eq!(ran.before, ran.after, "a window writes nothing");
    assert_eq!(
        ran.answer.asks,
        [serde_json::json!({ "route": "POST /verb-host/engine", "args": { "op": "showUi", "id": "melody:lead" } })]
    );
    assert_eq!(ran.answer.log, ["opening lead (vst3) on melody…"]);
    p.refused(
        "node external-ui ghost",
        "no external plugin module 'ghost' — try: lead",
    );
}
