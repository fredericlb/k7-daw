// modules_decode.rs — a module file decoded: the ported cases of moduleYaml, moduleTypes and builtinParams, and a hostile table.
// A reader would get wrong that a case pinning a message's WORDING is left out: findings are compared by place, never by sentence.

use song::Finding;
use song::modules::types::{
    completion_order, is_engine_ref, is_library_name, library_ref_parts, module_emits, module_status_of, slugify,
};
use song::modules::{Module, ModuleType, PortType, builtin_param_automatable, builtin_plugin, builtin_plugins};
use std::time::{Duration, Instant};

fn parse(text: &str) -> Result<Module, Finding> {
    song::modules::decode_module("module.k7x", text)
}

fn ok(text: &str) -> Module {
    parse(text).unwrap_or_else(|f| panic!("{text:?} refused: {} at {}:{}", f.message, f.line, f.col))
}

fn bad(text: &str) -> Finding {
    match parse(text) {
        Err(f) => f,
        Ok(_) => panic!("{text:?} was accepted"),
    }
}

// moduleYaml.test.ts, "validation": accepts a module the app actually writes.
#[test]
fn a_module_the_app_writes_reads_with_the_default_type() {
    let m = ok(
        "plugin({ type: \"internal\", name: \"Chorus\" })\nui({\n  cols: 4,\n  rows: 2,\n  widgets: { w1: { type: \"knob\", col: 0, row: 0, label: \"Depth\", bind: \"Depth\", min: 0, max: 1 } },\n})\n",
    );
    assert_eq!(
        m.ui.as_ref()
            .and_then(|u| u.widgets["w1"].bind.as_ref())
            .map(|b| b.0.as_str()),
        Some("Depth")
    );
    assert_eq!(m.module_type, Some(ModuleType::Plugin));
}

// moduleYaml.test.ts, "validation": reads an empty file as an empty module.
#[test]
fn an_empty_file_is_an_empty_module() {
    assert_eq!(ok("").module_type, Some(ModuleType::Plugin));
}

// moduleYaml.test.ts, "validation": leaves the type unstated when the file extends.
#[test]
fn a_module_that_extends_keeps_its_type_unstated() {
    assert_eq!(ok("extends(\"@workspace/instruments/kit\")\n").module_type, None);
    assert_eq!(ok("io(\"out\")\n").module_type, None);
}

// moduleYaml.test.ts, "validation": refuses a `label`.
#[test]
fn a_label_is_an_unknown_key_on_its_key() {
    let f = bad("label(\"Grande salle\")\nplugin({ type: \"internal\", name: \"Reverb\" })\n");
    assert_eq!((f.line, f.col), (1, 1));
}

// moduleYaml.test.ts, "validation": catches the typo a closed struct is for, and says where.
#[test]
fn a_typo_is_found_at_its_line() {
    let f = bad("plugin({ type: \"internal\" })\nparamz({})\n");
    assert_eq!((f.line, f.col), (2, 1));
    assert!(f.message.contains("paramz"));
}

// moduleYaml.test.ts, "validation": reports a syntax error at its line and column.
#[test]
fn a_syntax_error_is_located() {
    let f = bad("plugin({ type: \"internal\" })\nui(\n");
    assert!(f.line >= 2);
}

// moduleYaml.test.ts, "validation": reports a widget field that belongs to another type, on its line.
#[test]
fn a_widget_field_of_another_type_is_found_on_its_key() {
    let f = bad(
        "ui({\n  cols: 1,\n  rows: 1,\n  widgets: {\n    w2: { type: \"toggle\", col: 0, row: 0, bind: \"Bypass\", min: 0 },\n  },\n})\n",
    );
    assert_eq!(f.line, 5);
}

// moduleYaml.test.ts, "validation": accepts the asset dropdown that started all this.
#[test]
fn the_asset_dropdown_reads() {
    ok(
        "params({ model: \"jcm800.nam\" })\nui({\n  cols: 4,\n  rows: 2,\n  widgets: { w2: { type: \"dropdown\", col: 0, row: 0, bind: \"model\", library: \"model\" } },\n})\n",
    );
}

// Decision 41 and moduleYaml.test.ts, "validation": tells a slice rack from a samples one.
#[test]
fn a_slice_rack_needs_its_source() {
    ok(
        "type(\"rack\")\nrack({ rack_type: \"slice\", source: \"break.wav\", symbols: { BD: { note: 36, from: 0, to: 0.5 } } })\n",
    );
    bad("type(\"rack\")\nrack({ rack_type: \"slice\", symbols: { BD: { note: 36, from: 0, to: 0.5 } } })\n");
    bad("rack({ rack_type: \"slice\", source: \"a.wav\", symbols: { BD: { note: 36 } } })\n");
    bad("rack({ rack_type: \"samples\", source: \"a.wav\" })\n");
}

// moduleYaml.test.ts, "validation": refuses pads written outside `symbols:`.
#[test]
fn pads_outside_symbols_are_refused_on_their_key() {
    let f = bad("type(\"rack\")\nrack({ Kick: { note: 36, sample: \"@k7-core/k.wav\" } })\n");
    assert_eq!((f.line, f.col), (2, 8));
    assert!(f.message.contains("Kick"));
}

// moduleYaml.test.ts, "validation": reads an empty rack as a kit with no pads of its own.
#[test]
fn an_empty_rack_is_a_kit_with_no_pads() {
    for rack in [
        "rack(null)\n",
        "rack({})\n",
        "rack({ symbols: null })\n",
        "rack({ rack_type: null })\n",
    ] {
        let m = ok(&format!("type(\"rack\")\n{rack}"));
        let json = serde_json::to_value(m.rack.expect("a rack")).expect("json");
        assert_eq!(
            json,
            serde_json::json!({ "rack_type": "samples", "symbols": {} }),
            "{rack}"
        );
    }
}

// moduleYaml.test.ts, "this app's vocabulary": keeps a note name and a param name with spaces.
#[test]
fn param_names_with_spaces_survive() {
    let m = ok("params({ \"Filter Freq\": 69, \"Transpose - Bypass\": 1 })\ncontent(\"C4\")\n");
    let json = serde_json::to_value(&m).expect("json");
    assert_eq!(json["params"]["Filter Freq"], 69.0);
    assert_eq!(json["params"]["Transpose - Bypass"], 1.0);
    assert_eq!(json["content"], "C4");
}

fn panel(tabs: &str) -> String {
    format!("ui({{ cols: 4, rows: 2, widgets: {{}}, tabs: [{tabs}] }})\n")
}

// moduleYaml.test.ts, "a panel's tabs": takes an ordered list of named pages.
#[test]
fn tabs_are_an_ordered_list_of_pages() {
    ok(&panel(
        "{ name: \"Osc\", widgets: {} }, { name: \"Filter\", widgets: {} }",
    ));
}

// moduleYaml.test.ts, "a panel's tabs": refuses a name past ten characters, and two tabs of one name.
#[test]
fn a_tab_name_is_short_and_unique() {
    bad(&panel("{ name: \"Oscillators\", widgets: {} }"));
    bad(&panel("{ name: \"Osc\", widgets: {} }, { name: \"Osc\", widgets: {} }"));
    // Trimmed first, then counted in UTF-16 units: five emoji are ten.
    let m = ok(&panel("{ name: \"  Osc  \", widgets: {} }"));
    assert_eq!(m.ui.expect("ui").tabs.expect("tabs")[0].name.0, "Osc");
    ok(&panel("{ name: \"😀😀😀😀😀\", widgets: {} }"));
    bad(&panel("{ name: \"😀😀😀😀😀😀\", widgets: {} }"));
    bad(&panel("{ name: \"   \", widgets: {} }"));
}

// moduleYaml.test.ts, "a panel's tabs": has no `tabs` WIDGET any more.
#[test]
fn there_is_no_tabs_widget() {
    bad("ui({ cols: 4, rows: 2, widgets: { t: { type: \"tabs\", col: 0, row: 0 } } })\n");
}

// The vertical layout must have the same tabs as the panel, in the same order.
#[test]
fn the_vertical_twin_has_the_same_tabs() {
    let tabs = "{ name: \"A\", widgets: {} }, { name: \"B\", widgets: {} }";
    ok(&format!(
        "ui({{ cols: 4, rows: 2, widgets: {{}}, tabs: [{tabs}], vertical: {{ cols: 2, rows: 4, widgets: {{}}, tabs: [{tabs}] }} }})\n"
    ));
    bad(
        "ui({ cols: 4, rows: 2, widgets: {}, tabs: [{ name: \"A\", widgets: {} }], vertical: { cols: 2, rows: 4, widgets: {} } })\n",
    );
}

// Each widget refinement of moduleSchema.ts, one finding on the key it is about.
#[test]
fn each_widget_refinement_is_found_on_its_key() {
    let w = |fields: &str| format!("ui({{ cols: 4, rows: 2, widgets: {{ w: {{ {fields} }} }} }})\n");
    for (fields, key) in [
        ("type: \"toggle\", col: 0, row: 0, min: 0", "min"),
        ("type: \"label\", col: 0, row: 0, curve: \"log\"", "curve"),
        ("type: \"knob\", col: 0, row: 0, text: \"x\"", "text"),
        ("type: \"lcdscreen\", col: 0, row: 0, bind: \"x\"", "bind"),
        ("type: \"slider\", col: 0, row: 0, bind: \"a\", pair: \"b\"", "pair"),
        ("type: \"knob\", col: 0, row: 0, bind: \"a\", pair: \"a\"", "pair"),
        ("type: \"knob\", col: 0, row: 0, color: \"#fff\"", "color"),
        ("type: \"knob\", col: 0, row: 0, invert: true", "invert"),
        ("type: \"knob\", col: 0, row: 0, source: \"input\"", "source"),
        ("type: \"knob\", col: 0, row: 0, options: []", "options"),
        ("type: \"knob\", col: 0, row: 0, library: \"x\"", "library"),
        // The key is absent, so the finding lands on the widget that lacks it.
        ("type: \"filepicker\", col: 0, row: 0", "w:"),
    ] {
        let text = w(fields);
        let f = bad(&text);
        let line = text.lines().nth(f.line as usize - 1).expect("a line");
        assert!(
            line.get(f.col as usize - 1..).is_some_and(|t| t.starts_with(key)),
            "{fields}: {f:?}"
        );
    }
    ok(&w(
        "type: \"knob\", col: 0, row: 0, bind: \"a\", pair: \"b\", min: 0, max: 1, step: 0.1, curve: \"lin\"",
    ));
    ok(&w(
        "type: \"group\", col: 0, row: 0, color: \"#a1b2c3d4\", invert: false",
    ));
    bad(&w("type: \"group\", col: 0, row: 0, color: \"#a1b2c\""));
    // A widget strips an unknown key like zod's object, so the panel reads.
    ok(&w("type: \"knob\", col: 0, row: 0, whatever: 1"));
}

// moduleSchema.ts: every regex of the schema, matched by hand.
#[test]
fn each_regex_of_the_schema_is_matched_by_hand() {
    ok("engine(\"tracktion\")\nengines({ tracktion: { params: { A: 1, B: \"x.nam\" } } })\n");
    bad("engine(\"a b\")\n");
    bad("engines({ \"a b\": { params: {} } })\n");
    bad("engines({ ok: { other: 1 } })\n");
    ok("shader(\"video-mixer_2\")\n");
    bad("shader(\"a/b\")\n");
    ok("model_url(\"https://example.com/m.nam\")\n");
    ok("model_url(\"http://x\")\n");
    bad("model_url(\"ftp://x\")\n");
    bad("model_url(\"https://\")\n");
    bad("model_url(\"https://a b\")\n");
    ok("strudel(\"algo/pattern.strudel\")\n");
    bad("strudel(\"algo/.strudel\")\n");
    bad("strudel(\"patterns/a.strudel\")\n");
    bad("strudel(\"algo/a.strudelx\")\n");
    ok("lut(\"looks/warm.CUBE\")\n");
    bad("lut(\"looks/warm.cub\")\n");
    ok("tap_outs({ BD: \"sub1\" })\nkit({ BD: 36, \"SN-2\": 38 })\n");
    bad("tap_outs({ \"B D\": \"sub1\" })\n");
    bad("kit({ \"B.D\": 36 })\n");
    bad("kit({ BD: 128 })\n");
}

// moduleSchema.ts: the unions zod leaves open, and the module-level ranges.
#[test]
fn the_open_unions_and_ranges_read() {
    let m =
        ok("channel([1, 2])\nmonitor(\"auto\")\nalternative(2)\nmix(0.5)\ninput_volume(-60)\nframe([0, 0, 1, 1])\n");
    let json = serde_json::to_value(&m).expect("json");
    assert_eq!(json["channel"], serde_json::json!([1, 2]));
    assert_eq!(json["monitor"], "auto");
    assert_eq!(json["alternative"], 2);
    ok("channel(\"Mic\")\nmonitor(true)\nalternative(\"dark\")\n");
    for text in [
        "channel([])\n",
        "channel([1, 2, 3])\n",
        "channel([1.5])\n",
        "channel(\"\")\n",
        "monitor(\"loud\")\n",
        "alternative(0)\n",
        "alternative(\"\")\n",
        "mix(1.5)\n",
        "input_volume(13)\n",
        "output_volume(-61)\n",
        "frame([0, 0, 1])\n",
        "params({ a: true })\n",
        "status(\"loud\")\n",
        "io(\"speaker\")\n",
        "type(\"gizmo\")\n",
        "inputs([{ name: \"\", type: \"audio\" }])\n",
        "inputs([{ name: \"in\", type: \"cv\" }])\n",
        "inputs([{ name: \"in\", type: \"audio\", extra: 1 }])\n",
        "automations([{ param: \"a\" }])\n",
        "plugin({ type: 5 })\n",
    ] {
        bad(text);
    }
    // The plugin block is loose: every other key of it is kept.
    let m = ok("plugin({ type: \"vst3\", name: \"Serum\", id: \"x\", nested: { a: [1, 2] } })\n");
    let plugin = serde_json::to_value(m.plugin).expect("json");
    assert_eq!(plugin["nested"]["a"][1], 2);
    let _ = PortType::Audio;
}

// A table of hostile inputs, each refused fast.
#[test]
fn hostile_module_files_are_refused_quickly() {
    let started = Instant::now();
    let deep = format!("content({}1{})\n", "[".repeat(300), "]".repeat(300));
    for text in [
        deep.as_str(),
        "ui({ cols: 1, rows: 1, widgets: { w: { type: \"knob\", col: 1e300, row: 0 } } })\n",
        "ui({ cols: 1, rows: 1, widgets: { w: { type: \"knob\", col: 0, row: 0, colSpan: 0 } } })\n",
        "ui({ cols: 1, rows: 1, widgets: { w: { type: \"knob\", col: -1, row: 0 } } })\n",
        "ui({ cols: 1.5, rows: 1, widgets: {} })\n",
        "rack({ symbols: { A: { note: 128 } } })\n",
        "rack({ symbols: { A: { note: 36, to: 0 } } })\n",
        "kit({ A: 99999999999999999999 })\n",
        "channel([0])\n",
        "alternative(1.5)\n",
        "alternative(-1)\n",
        "gain(\"loud\")\n",
        "gain(null)\n",
    ] {
        parse(text).expect_err(text);
    }
    // A nest the format allows is decoded, not recursed into without a stop.
    let nested = format!("plugin({{ x: {}1{} }})\n", "[".repeat(200), "]".repeat(200));
    ok(&nested);
    assert!(started.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
}

// builtinParams.test.ts: the table's shape, and the lookups.
#[test]
fn the_builtin_table_is_read_by_the_lookups() {
    let table = builtin_plugins();
    assert!(table.len() > 100, "{} built-ins", table.len());
    for (kind, p) in table {
        assert!(!p.name.is_empty(), "{kind}");
        assert!(
            ["instrument", "audioEffect", "midiEffect"].contains(&p.role.as_str()),
            "{kind}"
        );
        assert!(
            p.params
                .iter()
                .all(|x| !x.name.is_empty() && x.states.as_ref().is_none_or(|s| !s.is_empty())),
            "{kind}"
        );
    }
    assert!(builtin_plugin(Some("internal"), Some("4OSC")).is_some_and(|p| !p.params.is_empty()));
    // All three spellings reach it, and only `internal` does.
    assert!(builtin_plugin(Some("internal"), Some(" 4osc ")).is_some());
    assert!(builtin_plugin(Some("vst3"), Some("4OSC")).is_none());
    assert!(builtin_plugin(Some("internal"), None).is_none());
    assert!(!builtin_param_automatable("4osc", "Voices 1"));
    assert!(builtin_param_automatable("4osc", "Tune 1"));
    assert!(builtin_param_automatable("nosuchplugin", "x"));
    assert!(song::modules::builtin_plugin_for_ref("@engine/4osc").is_some());
    assert!(song::modules::builtin_plugin_for_ref("@workspace/4osc").is_none());
}

// moduleTypes.test.ts, "isEngineRef": claims the namespace and everything under it, and no look-alike.
#[test]
fn engine_refs_are_the_engine_namespace_alone() {
    assert!(is_engine_ref("@engine"));
    assert!(is_engine_ref("@engine/neural-amp-modeler"));
    assert!(is_engine_ref("@engine/audio/delay/adt"));
    assert!(!is_engine_ref("@engineering/audio/eq"));
    assert!(!is_engine_ref("@engines/x"));
    assert!(!is_engine_ref("@k7-core/audio/amp/nam"));
    assert!(!is_engine_ref("instruments/bass"));
    assert!(!is_engine_ref(""));
}

// moduleTypes.test.ts, "moduleStatusOf" and "completionOrder".
#[test]
fn status_defaults_to_listed_and_orders_the_completions() {
    assert_eq!(module_status_of(None), "listed");
    assert_eq!(module_status_of(Some("hidden")), "hidden");
    assert_eq!(module_status_of(Some("favorite")), "favorite");
    assert_eq!(module_status_of(Some("FAVORITE")), "listed");
    let status = |m: &&str| {
        module_status_of(
            m.strip_prefix("fav")
                .map(|_| "favorite")
                .or(m.strip_prefix("hid").map(|_| "hidden")),
        )
    };
    assert_eq!(
        completion_order(vec!["amp", "fav-bass", "hid-test", "verb"], status),
        ["fav-bass", "amp", "verb"]
    );
    assert_eq!(
        completion_order(vec!["fav-a", "b", "fav-c", "d", "fav-e"], status),
        ["fav-a", "fav-c", "fav-e", "b", "d"]
    );
    assert!(completion_order(vec!["hid-1", "hid-2"], status).is_empty());
}

// moduleTypes.ts: a plugin's slug, a library's name and the ref a library answers to.
#[test]
fn names_slugs_and_library_refs() {
    assert_eq!(slugify("Neural Amp Modeler"), "neural-amp-modeler");
    assert_eq!(slugify("  --A__B!!  "), "a-b");
    assert_eq!(slugify("***"), "");
    assert_eq!(slugify("ÉQ Pro 3"), "q-pro-3");
    assert!(is_library_name("k7-core"));
    assert!(!is_library_name("K7"));
    assert!(!is_library_name("meta"));
    assert!(!is_library_name("account"));
    assert!(!is_library_name(&"a".repeat(41)));
    assert_eq!(
        library_ref_parts("@k7-core/audio/reverb"),
        Some(("k7-core", "audio/reverb"))
    );
    assert_eq!(library_ref_parts("@account/audio/x"), Some(("account", "audio/x")));
    assert_eq!(library_ref_parts("@meta/doc"), None);
    assert_eq!(library_ref_parts("@k7-core/"), None);
    assert_eq!(module_emits(Some("io"), Some("camera"), &[]), Some(PortType::Video));
    assert_eq!(module_emits(Some("io"), Some("out"), &[]), None);
    assert_eq!(module_emits(Some("audio"), None, &[]), Some(PortType::Audio));
}

// Decision 42: a port's empty name is found by the module's check, on the `name` key.
#[test]
fn an_empty_port_name_is_found_on_its_key() {
    let f = bad("inputs([{ name: \"\", type: \"audio\" }])\n");
    assert_eq!((f.line, f.col), (1, 11));
    let f = bad("alternatives([{ outputs: [{ name: \"\", type: \"midi\" }] }])\n");
    assert_eq!(f.line, 1);
    ok("inputs([{ name: \"in\", type: \"audio\", mandatory: false }])\n");
}

// A samples rack's `source` is no field of it: the finding lands on that key, whatever the key order.
#[test]
fn a_samples_racks_source_is_refused_on_its_key() {
    for text in [
        "rack({ rack_type: \"samples\", source: \"a.wav\" })\n",
        "rack({ source: \"a.wav\", symbols: {} })\n",
        "alternatives([{ rack: { source: \"a.wav\" } }])\n",
    ] {
        let f = bad(text);
        assert_eq!(
            (f.line as usize, f.col as usize),
            (1, text.find("source").expect("a source") + 1),
            "{text}"
        );
    }
}
