// panels.rs — a module's panel, script and MIDI map beside its folder: uiLibrary.test.ts, controller.test.ts, decisions 61 to 64.
// A reader would get wrong that a panel that does not decode is a WARNING and no claim: a farther library's panel still wins.

use crate::support;

use song::files::Severity;
use song::modules::panels::{decode_controller, decode_ui_file, module_folder_for_ref, read_module_controller};
use song::resolve::{resolved_cc, resolved_ui};
use song::{Cache, MemSource};
use support::{Tmp, disk, disk_world, mem, mem_world, mem_world_roots, roots, world};

const MOD: &str = "plugin({ type: \"internal\", name: \"Amp\" })\n";
const PANEL: &str = "cols(2)\nrows(1)\nwidgets({ gain: { type: \"knob\", col: 0, row: 0, bind: \"gain\" } })\n";
const AMP: &str = "lib/k7-core/modules/audio/amp";

fn at(rel: &str) -> String {
    format!("/p/{rel}")
}

// uiLibrary.test.ts, "loads a module OUT of the media library, through the @k7-core token", and the loader overlay.
#[test]
fn a_panel_beside_a_module_is_stamped_on_it_and_on_its_nodes() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.k7x"), PANEL),
        (&format!("{AMP}/lead.k7x"), "params({ gain: 3 })\n"),
        ("ws/proj/nodes/mine.k7x", "extends(\"@k7-core/audio/amp\")\n"),
    ]);
    for key in ["@k7-core/audio/amp", "@k7-core/audio/amp:lead"] {
        assert!(w.vocab.modules[key].authored.ui.is_some(), "{key}");
    }
    // uiLibrary.test.ts, "shows a module's panel on the nodes that use it": by what the node extends (decision 63).
    assert!(w.nodes.nodes["mine"].authored.ui.is_some());
    assert!(resolved_ui(Some(&w.resolved.modules["mine"])).is_some());
}

// uiLibrary.test.ts, "replaces an inline ui whole rather than merging into it", "leaves modules alone when nothing has been drawn".
#[test]
fn a_panel_replaces_the_inline_ui_whole() {
    let inline = "ui({ cols: 9, rows: 9, widgets: { other: { type: \"knob\", col: 0, row: 0 } } })\n";
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), &format!("{MOD}{inline}")),
        (&format!("{AMP}/ui.k7x"), PANEL),
        ("lib/k7-core/modules/audio/bare/module.k7x", &format!("{MOD}{inline}")),
    ]);
    let ui = w.vocab.modules["@k7-core/audio/amp"].authored.ui.as_ref().expect("ui");
    assert_eq!((ui.cols.0, ui.widgets.len()), (2, 1));
    assert!(ui.widgets.contains_key("gain") && !ui.widgets.contains_key("other"));
    let bare = w.vocab.modules["@k7-core/audio/bare"]
        .authored
        .ui
        .as_ref()
        .expect("its own");
    assert_eq!(bare.cols.0, 9);
}

// uiLibrary.test.ts, "keeps two modules of the same name apart, one per level": the nearest library's file wins (decision 63).
#[test]
fn the_nearest_library_answers_and_levels_stay_apart() {
    let near = "cols(1)\nrows(1)\nwidgets({ near: { type: \"knob\", col: 0, row: 0 } })\n";
    let w = mem_world(&[
        ("lib/account/modules/audio/amp/module.k7x", MOD),
        ("lib/account/modules/audio/amp/ui.k7x", near),
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.k7x"), PANEL),
    ]);
    assert!(
        w.vocab.modules["@account/audio/amp"]
            .authored
            .ui
            .as_ref()
            .is_some_and(|u| u.widgets.contains_key("near"))
    );
    assert!(
        w.vocab.modules["@k7-core/audio/amp"]
            .authored
            .ui
            .as_ref()
            .is_some_and(|u| u.widgets.contains_key("gain"))
    );
}

// uiLibrary.test.ts, "reports an unreadable panel", and decision 61: a located warning, the module keeps its inline ui.
#[test]
fn a_panel_that_does_not_decode_warns_where_it_is_and_the_module_keeps_its_own() {
    let inline = "ui({ cols: 9, rows: 9, widgets: {} })\n";
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), &format!("{MOD}{inline}")),
        (&format!("{AMP}/ui.k7x"), "cols(2)\nrows(1)\nwidgets({})\nbogus(1)\n"),
        ("lib/k7-core/modules/audio/fine/module.k7x", MOD),
        ("lib/k7-core/modules/audio/fine/ui.k7x", PANEL),
    ]);
    let f: Vec<_> = w
        .vocab
        .findings
        .iter()
        .filter(|f| f.file == "@k7-core/audio/amp/ui.k7x")
        .collect();
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].severity, f[0].line), (Severity::Warning, 4));
    assert_eq!(
        w.vocab.modules["@k7-core/audio/amp"]
            .authored
            .ui
            .as_ref()
            .map(|u| u.cols.0),
        Some(9)
    );
    assert!(
        w.vocab.modules["@k7-core/audio/fine"].authored.ui.is_some(),
        "one bad panel hides no other"
    );
}

// Decisions 61 and 70: a project standing inside a library sees its `_modules/` as that library's, so two folders answer one ref.
#[test]
fn a_farther_librarys_panel_wins_over_a_nearer_one_that_will_not_decode() {
    let files = [
        ("ws/_modules/audio/amp/module.k7x", MOD),
        ("ws/_modules/audio/amp/ui.k7x", "widgets({})\n"),
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.k7x"), PANEL),
    ];
    let src = mem(&files);
    let r = song::Roots {
        workspace_namespace: Some("k7-core".into()),
        ..mem_world_roots()
    };
    let w = world(&src, &mut Cache::new(), &r).expect("a world");
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file == "@k7-core/audio/amp/ui.k7x" && f.severity == Severity::Warning)
    );
    assert!(w.vocab.panels.ui["@k7-core/audio/amp"].widgets.contains_key("gain"));
    // Without the host's answer the nearer level is `@workspace` and the two refs are apart (the default).
    let apart = mem_world(&files);
    assert!(apart.vocab.panels.ui.is_empty() || !apart.vocab.panels.ui.contains_key("@workspace/audio/amp"));
}

// uiLibrary.test.ts, "sees a panel added, edited and removed straight after a read": over MemSource, where a stamp is a generation.
#[test]
fn a_panel_added_edited_and_removed_moves_the_vocabulary_in_memory() {
    let mut src = mem(&[(&format!("{AMP}/module.k7x"), MOD)]);
    let (r, mut cache) = (mem_world_roots(), Cache::new());
    let ui_of = |c: &mut Cache, s: &MemSource| {
        c.vocabulary(s, &r).expect("vocabulary").modules["@k7-core/audio/amp"]
            .authored
            .ui
            .as_ref()
            .map(|u| u.cols.0)
    };
    assert_eq!(ui_of(&mut cache, &src), None);
    src.set(&at(&format!("{AMP}/ui.k7x")), Some(PANEL.to_owned()))
        .expect("set");
    assert_eq!(ui_of(&mut cache, &src), Some(2));
    src.set(&at(&format!("{AMP}/ui.k7x")), Some(PANEL.replace("cols(2)", "cols(4)")))
        .expect("set");
    assert_eq!(ui_of(&mut cache, &src), Some(4));
    src.set(&at(&format!("{AMP}/ui.k7x")), None).expect("remove");
    assert_eq!(ui_of(&mut cache, &src), None);
}

// The same on a disk tree aged past the settle window: a probed-absent path appearing rebuilds the vocabulary.
#[test]
fn a_panel_added_to_an_aged_disk_tree_rebuilds_the_vocabulary_and_an_unrelated_file_does_not() {
    let t = Tmp::new("panel-aged");
    t.write(&format!("{AMP}/module.k7x"), MOD);
    t.mkdir("ws/proj");
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let src = disk(&r);
    let first = cache.vocabulary(&src, &r).expect("vocabulary");
    t.write("ws/proj/notes.txt", "unrelated");
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &cache.vocabulary(&src, &r).expect("vocabulary")
    ));
    t.write(&format!("{AMP}/ui.k7x"), PANEL);
    t.age();
    let second = cache.vocabulary(&src, &r).expect("vocabulary");
    assert!(!std::sync::Arc::ptr_eq(&first, &second));
    assert!(second.modules["@k7-core/audio/amp"].authored.ui.is_some());
}

// uiLibrary.test.ts, "ignores a panel under a dot folder", "never reads a panel under a YAML name", "is not read as a node".
#[test]
fn a_dot_folder_a_yaml_name_and_a_node_do_not_make_a_panel() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.yaml"), "cols: 2\n"),
        ("lib/k7-core/modules/audio/.hid/module.k7x", MOD),
        ("lib/k7-core/modules/audio/.hid/ui.k7x", PANEL),
    ]);
    assert!(w.vocab.panels.ui.is_empty());
    assert!(!w.vocab.modules.keys().any(|k| k.ends_with(":ui")));
}

// uiLibrary.test.ts, "a panel's widgets.js": rides on the panel as text, reaches the nodes, is absent when the folder has none.
#[test]
fn a_script_rides_on_the_panel_and_survives_resolution() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.k7x"), PANEL),
        (&format!("{AMP}/widgets.js"), "draw()"),
        ("ws/proj/nodes/mine.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        ("lib/k7-core/modules/audio/plain/module.k7x", MOD),
        ("lib/k7-core/modules/audio/plain/ui.k7x", PANEL),
    ]);
    assert_eq!(
        w.vocab.panels.ui["@k7-core/audio/amp"].script.as_deref(),
        Some("draw()")
    );
    assert_eq!(
        resolved_ui(Some(&w.resolved.modules["mine"]))
            .and_then(|u| u.script)
            .as_deref(),
        Some("draw()")
    );
    assert_eq!(w.vocab.panels.ui["@k7-core/audio/plain"].script, None);
}

// Decision 62: `script` is never read from a panel file or an inline `ui:`; a panel is strict.
#[test]
fn a_script_key_is_refused_in_a_panel_file_and_stripped_from_an_inline_ui() {
    assert!(decode_ui_file("ui.k7x", &format!("{PANEL}script(\"x\")\n")).is_err());
    assert!(decode_ui_file("ui.k7x", &format!("{PANEL}extra(1)\n")).is_err());
    let m = support::module_of("audio", "a", "ui({ cols: 1, rows: 1, widgets: {}, script: \"x\" })\n");
    assert_eq!(m.authored.ui.expect("ui").script, None);
}

// Decision 62: a script over 1 MiB is absent with a warning; so is one that is not UTF-8 (disk only).
#[test]
fn a_big_script_and_a_non_utf8_one_are_absent_with_a_warning() {
    let big = "x".repeat(2 * 1024 * 1024);
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/ui.k7x"), PANEL),
        (&format!("{AMP}/widgets.js"), &big),
    ]);
    assert_eq!(w.vocab.panels.ui["@k7-core/audio/amp"].script, None);
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file == "@k7-core/audio/amp/widgets.js" && f.severity == Severity::Warning)
    );
    let t = Tmp::new("script-utf8");
    t.write(&format!("{AMP}/module.k7x"), MOD);
    t.write(&format!("{AMP}/ui.k7x"), PANEL);
    std::fs::write(t.0.join(AMP).join("widgets.js"), [0xff, 0xfe, 0x41]).expect("write");
    let g = disk_world(&t, &[]);
    assert_eq!(g.vocab.panels.ui["@k7-core/audio/amp"].script, None);
    assert!(
        g.vocab
            .findings
            .iter()
            .any(|f| f.file == "@k7-core/audio/amp/widgets.js")
    );
}

// controller.test.ts, "parseController": a hand-authored map; an empty file is an empty map (decision 64).
#[test]
fn a_map_reads_and_an_empty_file_is_an_empty_map() {
    let m = decode_controller("c.k7x", "gain({ ch: 1, cc: 7 })\n\"Filter Freq\"({ ch: 16, cc: 0 })\n").expect("a map");
    assert_eq!((m.0["gain"].ch, m.0["gain"].cc, m.0["Filter Freq"].ch), (1, 7, 16));
    assert!(decode_controller("c.k7x", "").expect("empty").0.is_empty());
}

// controller.test.ts, "refuses a channel outside 1-16 and a cc outside 0-127", "refuses a binding missing its channel".
#[test]
fn a_map_refuses_what_plays_nothing() {
    for bad in [
        "g({ ch: 0, cc: 1 })\n",
        "g({ ch: 17, cc: 1 })\n",
        "g({ ch: 1e300, cc: 1 })\n",
        "g({ ch: 1, cc: -1 })\n",
        "g({ ch: 1, cc: 127.5 })\n",
        "g({ ch: 1, cc: 128 })\n",
        "\"\"({ ch: 1, cc: 1 })\n",
        "g({ cc: 1 })\n",
        "g({ ch: 1, cc: 1, extra: 2 })\n",
    ] {
        let f = decode_controller("c.k7x", bad).expect_err(bad);
        assert!(f.line >= 1, "{bad}");
    }
}

const CTL: &str = "gain({ ch: 1, cc: 7 })\n";

// controller.test.ts, "reaches the module the loader hands on", "ignores a map the schema refuses rather than losing the module".
#[test]
fn a_map_is_stamped_and_a_broken_one_is_a_warning_that_loses_nothing() {
    let w = mem_world(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/controller.k7x"), CTL),
        ("lib/k7-core/modules/audio/bad/module.k7x", MOD),
        ("lib/k7-core/modules/audio/bad/controller.k7x", "g({ ch: 0, cc: 1 })\n"),
        ("lib/k7-core/modules/audio/empty/module.k7x", MOD),
        ("lib/k7-core/modules/audio/empty/controller.k7x", ""),
        ("ws/proj/nodes/mine.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        ("ws/proj/nodes/silent.k7x", "extends(\"@k7-core/audio/empty\")\n"),
    ]);
    assert_eq!(
        w.vocab.modules["@k7-core/audio/amp"].cc.as_ref().map(|c| c["gain"].cc),
        Some(7)
    );
    assert_eq!(
        resolved_cc(Some(&w.resolved.modules["mine"])).map(|c| c["gain"].ch),
        Some(1)
    );
    assert!(w.vocab.modules.contains_key("@k7-core/audio/bad") && w.vocab.modules["@k7-core/audio/bad"].cc.is_none());
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file == "@k7-core/audio/bad/controller.k7x" && f.severity == Severity::Warning)
    );
    assert_eq!(
        w.vocab.modules["@k7-core/audio/empty"].cc.as_ref().map(|c| c.len()),
        Some(0),
        "stamped as cc: {{}}"
    );
    assert!(
        resolved_cc(Some(&w.resolved.modules["silent"])).is_none(),
        "an empty map answers none"
    );
}

// controller.test.ts, `moduleFolderForRef`: the folder of a ref, none for an engine-catalogue ref; readModuleController's read half.
#[test]
fn the_folder_of_a_ref_and_its_map_are_found_in_the_first_library_that_holds_them() {
    let src = mem(&[
        (&format!("{AMP}/module.k7x"), MOD),
        (&format!("{AMP}/controller.k7x"), CTL),
        ("lib/account/modules/audio/other/module.k7x", MOD),
    ]);
    let (r, mut cache) = (mem_world_roots(), Cache::new());
    let v = cache.vocabulary(&src, &r).expect("vocabulary");
    assert_eq!(
        module_folder_for_ref(&src, &v.folders, "@k7-core/audio/amp")
            .expect("folder")
            .as_deref(),
        Some(at(AMP).as_str())
    );
    assert_eq!(
        module_folder_for_ref(&src, &v.folders, "@engine/some-plugin").expect("folder"),
        None
    );
    assert_eq!(
        read_module_controller(&src, &mut cache, &v.folders, "@k7-core/audio/amp")
            .expect("map")
            .0
            .len(),
        1
    );
    assert!(
        read_module_controller(&src, &mut cache, &v.folders, "@account/audio/other")
            .expect("map")
            .0
            .is_empty()
    );
}

// Decision 27: a bundle made after a vocabulary HIT equals one made after a miss: presets (length), panel and script (text), model probe.
#[test]
fn a_hits_bundle_equals_a_misss() {
    use song::DiskSource;
    use song::record::Recorder;
    let t = Tmp::new("panel-hit");
    t.write(
        &format!("{AMP}/module.k7x"),
        "plugin({ type: \"internal\", name: \"Amp\" })\nparams({ model: \"cached.nam\" })\n",
    );
    t.write(&format!("{AMP}/ui.k7x"), PANEL);
    t.write(&format!("{AMP}/widgets.js"), "draw()");
    t.write(&format!("{AMP}/amp.vstpreset"), "preset");
    t.write("home/models/cached.nam", "x");
    t.mkdir("ws/proj");
    t.age();
    let r = roots(&t);
    let (dirs, files) = r.mounts().expect("mounts");
    let src = DiskSource::with_files(&dirs, &files).expect("a source");
    let mut cache = Cache::new();
    let mut bundle = || {
        let rec = Recorder::new(&src);
        world(&rec, &mut cache, &r).expect("a world");
        rec.into_bundle_with_files(&dirs, &files)
    };
    let miss = bundle();
    let hit = bundle();
    assert_eq!(miss, hit);
    let script = &miss.files[&t.at(&format!("{AMP}/widgets.js"))];
    assert_eq!(script.text.as_deref(), Some("draw()"));
    assert!(miss.files[&t.at(&format!("{AMP}/amp.vstpreset"))].len.is_some());
    assert!(miss.files.contains_key(&t.at("home/models/cached.nam")));
    assert!(cache.vocabulary_hits() >= 1);
}

#[test]
fn the_widget_vocabulary_lists_are_the_enums_spelled_as_a_file_spells_them() {
    use song::modules::{WAVEFORM_SOURCES, WIDGET_TYPES, WidgetType};
    for name in WIDGET_TYPES {
        let t: WidgetType = serde_json::from_value(serde_json::json!(name)).expect(name);
        assert_eq!(serde_json::to_value(t).unwrap(), serde_json::json!(name));
    }
    let distinct: std::collections::HashSet<_> = WIDGET_TYPES.iter().collect();
    assert_eq!(distinct.len(), 14);
    assert_eq!(WAVEFORM_SOURCES, ["input", "output"]);
}
