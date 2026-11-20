// resolve_hostile.rs — the hostile table of 2c: each decoder and pass refuses its worst input, fast.
// A reader would get wrong what the limits count: the OUTPUT (widgets kept, bindings kept, merges made), never the input's bytes.

use crate::support;

use song::files::Severity;
use song::modules::panels::{decode_controller, decode_ui_file};
use std::time::{Duration, Instant};
use support::mem_world;

const AMP: &str = "lib/k7-core/modules/audio/amp";
const MOD: &str = "plugin({ type: \"internal\", name: \"Amp\" })\n";

fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = f();
    assert!(
        started.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "{what} took {:?}",
        started.elapsed()
    );
    out
}

// Panel: a nest of 300 is a syntax error of the parser's own (256), refused at the file.
#[test]
fn a_panel_nested_300_deep_is_refused() {
    let text = format!("cols(1)\nrows(1)\nwidgets({}1{})\n", "{a:".repeat(300), "}".repeat(300));
    let f = fast("a nest", || decode_ui_file("ui.k7x", &text).expect_err("refused"));
    assert!(f.line >= 1);
}

// Panel: a panel past the vocabulary's weight is not kept, and a warning at the module's ref says so.
#[test]
fn a_panel_of_too_many_widgets_is_dropped_with_a_warning() {
    let widgets: String = (0..100_100)
        .map(|i| format!("w{i}: {{ type: \"knob\", col: 0, row: 0 }},\n"))
        .collect();
    let panel = format!("cols(1)\nrows(1)\nwidgets({{\n{widgets}}})\n");
    let w = fast("200k widgets", || {
        mem_world(&[(&format!("{AMP}/module.k7x"), MOD), (&format!("{AMP}/ui.k7x"), &panel)])
    });
    assert!(w.vocab.panels.ui.is_empty());
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file.ends_with("/ui.k7x") && f.severity == Severity::Warning)
    );
}

// MIDI map: 200,000 bindings are decoded (the parser is linear) and refused by the vocabulary's weight.
#[test]
fn a_map_of_200000_bindings_is_dropped_with_a_warning() {
    let map: String = (0..200_000).map(|i| format!("p{i}({{ ch: 1, cc: 1 }})\n")).collect();
    assert!(fast("decode", || decode_controller("c.k7x", &map)).is_ok());
    let w = fast("vocabulary", || {
        mem_world(&[
            (&format!("{AMP}/module.k7x"), MOD),
            (&format!("{AMP}/controller.k7x"), &map),
        ])
    });
    assert!(w.vocab.panels.maps.is_empty());
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file.ends_with("/controller.k7x") && f.severity == Severity::Warning)
    );
}

// Extends: a cycle of two through a library node stops with a warning and terminates.
#[test]
fn a_cycle_through_a_library_node_terminates() {
    let w = mem_world(&[
        (
            "lib/k7-core/modules/audio/a/module.k7x",
            "extends(\"@k7-core/audio/b:y\")\nparams({ a: 1 })\n",
        ),
        ("lib/k7-core/modules/audio/b/module.k7x", "params({ b: 1 })\n"),
        (
            "lib/k7-core/modules/audio/b/y.k7x",
            "extends(\"@k7-core/audio/a\")\nparams({ y: 1 })\n",
        ),
    ]);
    assert!(w.resolved_vocab.modules.contains_key("@k7-core/audio/a"));
    assert!(
        w.resolved_vocab
            .findings
            .iter()
            .any(|f| f.severity == Severity::Warning)
    );
}

// Files: an absolute model, a library ref climbing out, a base climbing out and a NUL are no file, with no warning and no panic.
#[test]
fn hostile_file_refs_are_no_file() {
    let w = mem_world(&[
        (
            &format!("{AMP}/module.k7x"),
            "params({ model: \"/etc/passwd\", ir: \"@k7-core/../../x\", onnx: \"a\\u0000b.onnx\" })\nbase(\"../../../x\")\n",
        ),
        ("lib/k7-core/irs/halls/big.wav", "x"),
    ]);
    let m = &w.vocab.modules["@k7-core/audio/amp"];
    assert_eq!(
        (
            m.model.as_deref(),
            m.ir.as_deref(),
            m.onnx.as_deref(),
            m.preset.as_deref()
        ),
        (None, None, None, None)
    );
    assert!(w.vocab.findings.is_empty(), "{:?}", w.vocab.findings);
}

// Files: a `\` in a base is a character of a name on Unix (decision 1).
#[cfg(unix)]
#[test]
fn a_backslash_in_a_base_is_a_character_of_the_name() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/k.k7x"), "base(\"a\\\\b\")\n"),
        (&format!("{AMP}/base/a\\b.vstpreset"), "x"),
    ]);
    assert_eq!(
        w.vocab.modules["@k7-core/audio/amp:k"].preset.as_deref(),
        Some("/p/lib/k7-core/modules/audio/amp/base/a\\b.vstpreset")
    );
}

// Files: on Windows a `\` in a base is `Outside`: the defaults answer and a warning says so (decision 1, 65).
#[cfg(windows)]
#[test]
fn a_backslash_in_a_base_is_outside_on_windows() {
    let t = support::Tmp::new("hostile-bs");
    t.write(&format!("{AMP}/module.k7x"), MOD);
    t.write(&format!("{AMP}/k.k7x"), "base(\"a\\\\b\")\n");
    let w = support::disk_world(&t, &[]);
    assert_eq!(w.vocab.modules["@k7-core/audio/amp:k"].preset, None);
    assert!(w.vocab.findings.iter().any(|f| f.severity == Severity::Warning));
    assert!(
        song::roots::is_absolute(r"C:\x", song::PathRule::Windows)
            && song::roots::is_absolute("/x", song::PathRule::Windows)
    );
}

// Files: 10,000 alternatives each with a rack are resolved once each and the whole stays fast.
#[test]
fn ten_thousand_alternatives_each_with_a_rack_stay_fast() {
    let alts: String = (0..10_000)
        .map(|i| format!("{{ name: \"a{i}\", rack: {{ symbols: {{ A: {{ note: 1, clip: \"c{i}.wav\" }} }} }} }},"))
        .collect();
    let node = format!("alternatives([{alts}])\nalternative(\"a9999\")\n");
    let w = fast("alternatives", || mem_world(&[("ws/proj/nodes/n.k7x", &node)]));
    let rack = w.resolved.modules["n"]
        .authored
        .rack
        .as_ref()
        .expect("the chosen set's rack");
    assert_eq!(
        rack.symbols["A"].clip.as_ref().map(|c| c.0.as_str()),
        Some("/p/ws/proj/nodes/c9999.wav")
    );
}
