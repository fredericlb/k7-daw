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
    let ran = p.ok_answered("system channel type 4 stereo", |c| {
        assert_eq!(c["route"], "POST /verb-host/devices/stereo");
        assert_eq!(c["args"], json!({ "channel": 4, "stereo": true }));
        json!({ "heard": true })
    });
    assert_eq!(ran.answer.log, ["in 3+4 is one stereo device"]);
    assert_eq!(prefs(&p).input_pairs, [3.0]);
    let unheard = p.ok_answered("system channel type 3 mono", |_| json!({ "heard": false }));
    assert_eq!(
        unheard.answer.log,
        ["in 3+4 is two mono devices — remembered; the engine hears it on its next connect"]
    );
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
fn the_separating_chains_are_checked_and_remembered() {
    let mut p = Project::open("mute-solo").with_core();
    p.refused("system stems", "nothing on this machine will separate a record");
    let ran = p.ok("system stems @k7-core/stems");
    assert_eq!(
        ran.answer.log,
        ["stems are separated with @k7-core/stems — separates with @k7-core/cold/stemsplit"]
    );
    p.refused("system stems @k7-core/process/normalize", "does not separate");
    p.refused("system instrumental @k7-core/stems", "writes no instrumental");
    p.refused(
        "system stems @k7-core/ghost",
        "system stems: no chain '@k7-core/ghost' — known here:",
    );
    assert_eq!(prefs(&p).stems_chain.as_deref(), Some("@k7-core/stems"));
    p.refused("system stems", "currently: @k7-core/stems");
    p.ok("system stems ''");
    assert_eq!(prefs(&p).stems_chain, None);
    let inst = p.ok("system instrumental @k7-core/instrumental");
    assert_eq!(
        inst.answer.log,
        ["the instrumental comes from @k7-core/instrumental — written by @k7-core/cold/instrumental"]
    );
    assert_eq!(prefs(&p).instrumental_chain.as_deref(), Some("@k7-core/instrumental"));
    let cleared = p.ok("system instrumental \"\"");
    assert_eq!(cleared.answer.log, ["the instrumental chain is cleared"]);
}

#[test]
fn onnx_providers_are_set_on_the_machine() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok_answered("system onnx DML,cpu", |c| {
        assert_eq!(c["route"], "POST /verb-host/onnx");
        assert_eq!(c["args"], json!({ "set": ["dml", "cpu"] }));
        json!({ "providers": ["dml", "cpu"] })
    });
    assert_eq!(ran.answer.log.len(), 2, "the dml warning");
    assert_eq!(ran.before, ran.after);
    let cpu = p.ok_answered("system onnx \"\"", |c| {
        assert_eq!(c["args"], json!({ "set": [] }));
        json!({ "providers": [] })
    });
    assert_eq!(cpu.answer.log, ["separation goes back to the CPU"]);
    p.refused("system onnx tpu", "unknown execution provider(s): tpu");
    let now = p.answered("system onnx", |c| {
        assert_eq!(c["args"], json!({}));
        json!({ "providers": ["coreml", "cpu"] })
    });
    assert_eq!(
        now.answer.refuse[1],
        "currently: coreml, cpu — pass \"\" to go back to cpu"
    );
    let none = p.answered("system onnx", |_| json!({ "providers": [] }));
    assert!(none.answer.refuse[1].starts_with("nothing is set"));
    // No engine chosen and no machine reached are the verb's own refusal, not a throw.
    for code in ["off", "no-machine"] {
        let off = p.answered("system onnx cpu", |_| json!({ "error": "x", "code": code }));
        assert_eq!(
            off.answer.refuse,
            ["system onnx: no engine is chosen — the setting lives on its machine"]
        );
        assert_eq!(off.answer.threw, None);
    }
    let broke = p.answered(
        "system onnx cpu",
        |_| json!({ "error": "the machine said no", "code": "busy" }),
    );
    assert_eq!(broke.answer.threw.as_deref(), Some("the machine said no"));
}

#[test]
fn plugins_are_listed_and_scanned_by_the_engine_route() {
    let mut p = Project::open("mute-solo");
    let synth = json!({ "name": "Ext Synth", "format": "VST3", "manufacturer": "Acme", "role": "instrument" });
    let fx = json!({ "name": "Verb", "format": "VST3", "manufacturer": "Acme", "role": "audioFX" });
    let list = p.ok_answered("system plugins list vst3", |c| {
        assert_eq!(c["route"], "GET /verb-host/plugins");
        assert_eq!(c["args"], json!({ "filter": "vst3" }));
        json!({ "plugins": [synth, fx] })
    });
    assert_eq!(
        list.answer.log,
        ["  [inst] Ext Synth  [VST3]  Acme", "        Verb  [VST3]  Acme"]
    );
    let all = p.ok_answered("system plugins list", |c| {
        assert_eq!(c["args"], json!({}));
        json!({ "plugins": [] })
    });
    assert_eq!(all.answer.log, ["no plugins cached — run: k7 engine scan"]);
    let scan = p.ok_answered("system plugins scan", |c| {
        assert_eq!(c["route"], "POST /verb-host/plugins/rescan");
        json!({ "plugins": [synth] })
    });
    assert_eq!(
        scan.answer.log,
        [
            "rescanning plugins… (first scan can take a while)",
            "scanned 1 plugin(s)"
        ]
    );
    assert_eq!(scan.before, scan.after);
}

fn run_without_project(fs: &mut MemFs, line: &str) -> Answer {
    let req = VerbRequest {
        line: line.to_owned(),
        project: String::new(),
        env: json!({ "studioHome": "/home-none", "metaRoot": "/k7/meta" }),
        answers: Vec::new(),
        stamp: None,
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
