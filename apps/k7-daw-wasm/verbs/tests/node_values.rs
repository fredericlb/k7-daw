// node_values.rs — a node's values: `node param` in its branches, `module cc`, presets and engines, on real fixtures.
// A reader would get wrong where a preset or a MIDI map lands: outside the project, in the workspace, which undo still restores.

mod support;

use k7x::edit::Value;
use song_edit::Fs;
use support::Project;

fn put(p: &mut Project, abs: &str, text: &str) {
    let dir = abs.rsplit_once('/').map_or("", |(d, _)| d);
    p.fs.mkdir_all(dir).expect("a folder");
    p.fs.write_atomic(abs, text.as_bytes()).expect("a write");
}

fn lead_param(p: &mut Project, name: &str) -> Option<f64> {
    p.track("melody").plugins[0].params.get(name).copied()
}

#[test]
fn a_plugin_param_is_written_and_pushed() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip(r#"node param lead "Filter Freq" 69"#);
    assert_eq!(ran.answer.log, ["lead: Filter Freq = 69"]);
    assert_eq!(ran.answer.effects, ["applyProject"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/lead.k7x"]);
    p.ok("node param lead Filter Freq 69");
    assert_eq!(lead_param(&mut p, "Filter Freq"), Some(69.0));
    p.refused("node param lead Level loud", "\"loud\" is not a number");
    p.refused("node param ghost Level 1", "no plugin module 'ghost' — try: lead");
    p.refused("node param lead", "usage: node param");
}

#[test]
fn out_s_volume_is_the_project_s_master_volume() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("node param OUT Volume -3");
    assert_eq!(ran.answer.log, ["OUT: Volume = -3"]);
    assert_eq!(ran.changed(&p.dir), ["project.k7x"]);
    p.ok("node param OUT Volume -3");
    assert_eq!(p.doc().master_volume, -3.0);
    p.ok("node param OUT Volume 0");
    assert!(
        p.data("project.k7x")
            .get("project")
            .and_then(|v| v.get("masterVolume"))
            .is_none()
    );
    p.refused("node param OUT Volume 20", "outside the fader");
    p.refused("node param OUT Gain 1", "OUT has one param, Volume");
    p.refused("node param MIX Volume 1", "MIX is the mixer's sum");
    p.refused("node param CUE Volume 1", "CUE carries no level");
}

#[test]
fn an_intrinsic_is_the_module_s_own_field() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("node param lead @mix 0.25");
    assert_eq!(ran.answer.log, ["lead: mix 25% wet"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/lead.k7x"]);
    let ran = p.round_trip("node param lead @output-volume -6");
    assert_eq!(ran.answer.log, ["lead: output volume -6 dB"]);
    p.refused("node param lead @mix 2", "is not a mix between 0 and 1");
    p.refused("node param lead @colour red", "no intrinsic '@colour'");
}

#[test]
fn a_file_param_names_one_of_the_module_s_assets() {
    let mut p = Project::open("note-lane");
    p.refused("node param lead model amp.nam", "lead carries no assets");
    put(&mut p, "/w/ws/_modules/instruments/fourosc/assets/amp.nam", "{}");
    let ran = p.round_trip("node param lead model amp.nam");
    assert_eq!(ran.answer.log, ["lead: amp.nam"]);
    p.refused(
        "node param lead model other.nam",
        "no 'other.nam' in lead's assets — it has: amp.nam",
    );
    p.refused(
        "node param lead ir @k7-core/irs/hall.wav",
        "no library has a irs/ folder",
    );
}

#[test]
fn a_cc_is_bound_in_the_module_s_map() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("module cc lead Filter Freq 1 74");
    assert_eq!(
        ran.answer.log,
        [
            "lead: Filter Freq <- ch1 cc74  (../_modules/instruments/fourosc/controller.k7x)",
            "@workspace/instruments/fourosc's map — shared by every node of it",
        ]
    );
    p.ok("module cc lead Filter Freq 1 74");
    let map =
        song_edit::workspace_doc::ws_data(&p.fs, "/w/ws/_modules/instruments/fourosc/controller.k7x").expect("a map");
    assert_eq!(
        map.get("Filter Freq").and_then(|b| b.get("cc")),
        Some(&Value::Num(74.0))
    );
    let ran = p.ok("module cc lead Filter Freq none");
    assert_eq!(
        ran.answer.log[0],
        "lead: Filter Freq unbound  (../_modules/instruments/fourosc/controller.k7x)"
    );
    p.refused("module cc lead Cutoff 17 1", "channel 17 is not 1-16");
    p.refused("module cc lead Cutoff 1 128", "cc 128 is not 0-127");
    p.refused("module cc ghost Cutoff 1 1", "no module \"ghost\" in this project");
    p.refused("module cc lead", "usage: module cc");
}

#[test]
fn a_preset_is_saved_listed_applied_and_updated() {
    let mut p = Project::open("note-lane");
    let ran = p.ok("node preset apply lead");
    assert_eq!(
        ran.answer.log,
        ["lead (@workspace/instruments/fourosc) has no presets — 'node preset save lead <name>' keeps one"]
    );
    p.ok("node param lead Level -6");
    let ran = p.round_trip(r#"node preset save lead "Soft Lead""#);
    assert_eq!(
        ran.answer.log,
        ["lead: saved preset 'Soft Lead' for @workspace/instruments/fourosc in workspace"]
    );
    assert!(
        ran.changed(&p.dir).iter().all(|f| f.starts_with("/w/ws/")),
        "{:?}",
        ran.changed(&p.dir)
    );
    p.ok(r#"node preset save lead "Soft Lead""#);
    p.refused(r#"node preset save lead "Soft Lead""#, "node preset save:");
    let ran = p.ok("node preset apply lead");
    assert!(
        ran.answer.log[0].ends_with("workspace:Soft Lead"),
        "{:?}",
        ran.answer.log
    );
    p.ok("node param lead Level -20");
    let ran = p.round_trip(r#"node preset apply lead workspace "Soft Lead""#);
    assert_eq!(ran.answer.log, ["lead: preset 'Soft Lead' (workspace)"]);
    assert_eq!(ran.answer.effects, ["applyProject"]);
    p.ok(r#"node preset apply lead workspace "Soft Lead""#);
    assert_eq!(lead_param(&mut p, "Level"), Some(-6.0));
    p.ok("node param lead Level -9");
    let ran = p.round_trip(r#"node preset update lead workspace "Soft Lead""#);
    assert_eq!(ran.answer.log, ["lead: updated preset 'Soft Lead' in workspace"]);
    p.refused(r#"node preset update lead workspace "Ghost""#, "node preset update:");
    p.refused(r#"node preset apply lead workspace "Ghost""#, "node preset:");
}

#[test]
fn an_engine_is_listed_and_switched() {
    let mut p = Project::open("note-lane");
    put(
        &mut p,
        "/w/ws/_modules/instruments/macro/module.k7x",
        "engine(\"plaits\")\n",
    );
    put(
        &mut p,
        "/w/ws/_modules/instruments/macro/engines/plaits/module.k7x",
        "extends(\"@engine/mu-plaits\")\n",
    );
    put(
        &mut p,
        "/w/ws/_modules/instruments/macro/engines/braids/module.k7x",
        "extends(\"@engine/mu-braids\")\n",
    );
    let lead = format!("{}/nodes/lead.k7x", p.dir);
    put(&mut p, &lead, "extends(\"@workspace/instruments/macro\")\n");
    let ran = p.ok("node engine lead");
    assert_eq!(ran.answer.log, ["lead: braids  [plaits]"]);
    let ran = p.round_trip("node engine lead braids");
    assert_eq!(ran.answer.log, ["lead: running engine 'braids'"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/lead.k7x"]);
    p.ok("node engine lead braids");
    assert!(p.text("nodes/lead.k7x").contains("braids"));
    p.refused("node engine lead ghost", "node engine:");
    let ran = p.ok("node engine melody");
    assert_eq!(ran.answer.log, ["melody has no engines"]);
}
