// system.rs — the machine's verbs over a real fixture: the preferences file they write under studioHome, read back, and what they ask.
// A reader would get wrong that a system verb needs a project: it runs with `project: ""` too, as the TypeScript's do.

mod support;

use serde_json::json;
use song_edit::preferences::{CueOutput, parse_preferences};
use song_edit::{Fs, MemFs};
use support::Project;
use verbs::{Answer, VerbRequest, run_request};

const PREFS: &str = "/home-none/preferences.k7x";

fn prefs(p: &Project) -> song_edit::preferences::Preferences {
    parse_preferences(&p.fs.text(PREFS).unwrap_or_default())
}

/// Undoes `ran` and checks the preferences file is gone again, as it was absent before.
fn undone(p: &mut Project, ran: &support::Ran) {
    p.undo(ran);
    assert_eq!(p.files(), ran.before);
}

#[test]
fn channel_labels_are_set_listed_and_cleared() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok("system channel set in 3+4 synth pad");
    assert_eq!(ran.answer.log, ["in 3+4 = synth pad"]);
    assert_eq!(ran.changed(&p.dir), [PREFS.to_owned()]);
    undone(&mut p, &ran);
    p.ok("system channel set in 3+4 synth pad");
    p.ok("system channel set OUT 1+2 phones");
    let got = prefs(&p);
    assert_eq!(got.channels_in[0].ch, [3.0, 4.0]);
    assert_eq!(got.channels_in[0].label, "synth pad");
    assert_eq!(got.channels_out[0].label, "phones");
    let list = p.ok("system channel list");
    assert_eq!(list.answer.log[1], "in:  3+4 synth pad");
    assert_eq!(list.answer.log[2], "out: 1+2 phones");
    assert_eq!(list.before, list.after);
    p.ok("system channel clear in 3+4");
    assert!(prefs(&p).channels_in.is_empty());
    p.refused("system channel set in x synth", "bad channels 'x'");
    p.refused("system channel set sideways 1 a", "usage: system channel set");
    p.refused("system channel clear in", "usage: system channel clear");
}

#[test]
fn a_pair_is_remembered_and_the_engine_asked() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok("system channel type 4 stereo");
    assert_eq!(
        ran.answer.asks,
        [json!({ "member": "setInputStereo", "args": [4, true] })]
    );
    assert!(ran.answer.log[0].starts_with("in 3+4 is one stereo device"));
    assert_eq!(prefs(&p).input_pairs, [3.0]);
    p.ok("system channel type 3 mono");
    assert!(prefs(&p).input_pairs.is_empty());
    p.refused("system channel type 0 mono", "is not a channel");
    p.refused("system channel type 3 quad", "usage: system channel type");
}

#[test]
fn the_cue_pair_takes_numbers_or_a_known_label() {
    let mut p = Project::open("mute-solo");
    p.refused("system cue", "nothing is set");
    let ran = p.ok("system cue 3+4");
    assert_eq!(ran.answer.log, ["CUE leaves by channel(s) 3+4"]);
    assert_eq!(prefs(&p).cue_output, Some(CueOutput::Channels(vec![3.0, 4.0])));
    p.refused("system cue", "currently: 3+4");
    p.refused("system cue phones", "no output labelled 'phones'");
    p.ok("system channel set out 5+6 phones");
    let named = p.ok("system cue phones");
    assert_eq!(named.answer.log, ["CUE leaves by 'phones' (channel(s) 5+6)"]);
    assert_eq!(prefs(&p).cue_output, Some(CueOutput::Label("phones".to_owned())));
    p.ok("system cue \"\"");
    assert_eq!(prefs(&p).cue_output, None);
}

#[test]
fn the_separating_chains_are_remembered_unchecked() {
    let mut p = Project::open("mute-solo");
    p.refused("system stems", "nothing on this machine will separate a record");
    let ran = p.ok("system stems @k7-core/stems");
    assert!(ran.answer.log[0].starts_with("stems are separated with @k7-core/stems — not checked"));
    assert_eq!(prefs(&p).stems_chain.as_deref(), Some("@k7-core/stems"));
    p.refused("system stems", "currently: @k7-core/stems");
    p.ok("system stems ''");
    assert_eq!(prefs(&p).stems_chain, None);
    p.ok("system instrumental @k7-core/instrumental");
    assert_eq!(prefs(&p).instrumental_chain.as_deref(), Some("@k7-core/instrumental"));
    let cleared = p.ok("system instrumental \"\"");
    assert_eq!(cleared.answer.log, ["the instrumental chain is cleared"]);
}

#[test]
fn onnx_providers_are_asked_of_the_machine() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok("system onnx DML,cpu");
    assert_eq!(
        ran.answer.asks,
        [json!({ "member": "onnxProviders", "args": [["dml", "cpu"]] })]
    );
    assert_eq!(ran.answer.log.len(), 2, "the dml warning");
    assert_eq!(ran.before, ran.after);
    let cpu = p.ok("system onnx \"\"");
    assert_eq!(cpu.answer.asks[0]["args"], json!([[]]));
    p.refused("system onnx tpu", "unknown execution provider(s): tpu");
    p.refused("system onnx", "(onnxProviders)");
}

#[test]
fn plugins_are_the_hosts_to_list_and_scan() {
    let mut p = Project::open("mute-solo");
    let list = p.ok("system plugins list vst3");
    assert_eq!(list.answer.asks, [json!({ "member": "listPlugins", "args": ["vst3"] })]);
    let all = p.ok("system plugins list");
    assert_eq!(all.answer.asks[0]["args"], json!([null]));
    let scan = p.ok("system plugins scan");
    assert_eq!(scan.answer.asks, [json!({ "member": "rescanPlugins", "args": [] })]);
    assert_eq!(scan.before, scan.after);
}

fn run_without_project(fs: &mut MemFs, line: &str) -> Answer {
    let req = VerbRequest {
        line: line.to_owned(),
        project: String::new(),
        env: json!({ "studioHome": "/home-none", "metaRoot": "/k7/meta" }),
    };
    run_request(fs, &mut || "id".to_owned(), &req)
}

#[test]
fn a_system_verb_runs_with_no_project_open() {
    let mut fs = MemFs::new();
    let a = run_without_project(&mut fs, "system stems @k7-core/stems");
    assert!(!a.refused, "{:?}", a.refuse);
    assert_eq!(
        a.log,
        ["stems are separated with @k7-core/stems — not checked: no project is open to resolve it against"]
    );
    assert!(fs.text(PREFS).unwrap().contains("@k7-core/stems"));
    let b = run_without_project(&mut fs, "system channel set in 1 mic");
    assert_eq!(b.log, ["in 1 = mic"]);
    let c = run_without_project(&mut fs, "system channel list");
    assert_eq!(c.log[1], "in:  1 mic");
}

#[test]
fn a_preferences_file_that_does_not_read_is_never_saved_over() {
    let mut p = Project::open("mute-solo");
    p.fs.mkdir_all("/home-none").unwrap();
    p.fs.write_atomic(PREFS, b"this is not k7x ((").unwrap();
    let ran = p.run("system channel set in 1 mic");
    assert!(ran.answer.refused);
    assert!(ran.answer.threw.as_deref().unwrap_or("").contains("does not read"));
    assert_eq!(ran.before, ran.after);
}
