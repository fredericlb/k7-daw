// console.rs — the summing console (decisions 279, 280): the systems a library holds, the two halves as plugins, the slots 2h places them in.
// A reader would get wrong that nothing is placed here: the halves are returned for 2h to clone per track.

use crate::lanes_support;
use crate::support;

use song::console::{
    CONSOLE_HALVES, CONSOLE_ID, HalfPlugin, channel_slot, console_of, console_systems, engine_plugin, takes_channel,
};
use song::lanes::fault::FaultKind;
use song::modules::{LoadedModule, ModuleKind, Plugin, Vocabulary, base_module_refs, build_vocabulary};
use song::{Cache, Severity};
use support::{mem, mem_world_roots};

const SYS: &str = "@k7-core/console/console-6";

fn world(extra: &[(&str, String)]) -> (song::MemSource, Vocabulary) {
    let mut files: Vec<(&str, String)> = vec![
        (
            "lib/k7-core/modules/console/console-6/channel/module.k7x",
            "plugin({ type: \"internal\", name: \"Console 6 Channel\" })\nparams({ Drive: 0.5, Mode: \"warm\", Trim: -3 })\n".into(),
        ),
        (
            "lib/k7-core/modules/console/console-6/buss/module.k7x",
            "plugin({ type: \"internal\", name: \"Console 6 Buss\" })\n".into(),
        ),
        (
            "lib/k7-core/modules/console/broken/channel/module.k7x",
            "plugin({ type: \"internal\", name: \"X\" })\n".into(),
        ),
    ];
    files.extend(extra.iter().map(|(a, b)| (*a, b.clone())));
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let src = mem(&refs);
    let v = build_vocabulary(&src, &mut Cache::new(), &mem_world_roots()).expect("vocabulary");
    (src, v)
}

// console.test.ts "lists a system only when it has both halves, named by its folder" and "never offers a half as a module".
#[test]
fn a_system_is_listed_only_with_both_halves() {
    let (_, v) = world(&[]);
    let systems = console_systems(&v);
    assert_eq!(systems.len(), 1);
    assert_eq!(
        (systems[0].ref_.as_str(), systems[0].label.as_str()),
        (SYS, "console-6")
    );
    assert!(base_module_refs(&v.modules).iter().all(|r| !r.contains("/console/")));
    assert_eq!(serde_json::to_value(&systems[0]).expect("json")["ref"], SYS);
}

// 279: sorted by label in code points; a regexp read by hand: three segments exactly, `@` and a name, no `channel` inside.
#[test]
fn systems_are_sorted_by_label_and_the_shape_is_exact() {
    let (_, mut v) = world(&[]);
    let mut add = |r: &str| {
        v.modules
            .insert(r.to_owned(), LoadedModule::bare(ModuleKind::Plugin, "console", "x"));
    };
    for s in [
        "@a/console/zeta",
        "@b/console/alpha",
        "@c/console/Beta",
        "@d/console/only-channel",
    ] {
        add(&format!("{s}/channel"));
        if s != "@d/console/only-channel" {
            add(&format!("{s}/buss"));
        }
    }
    // Shapes the pattern refuses: a deeper system, a library of no name, another type folder, a bare channel.
    for r in [
        "@a/console/x/y/channel",
        "@/console/x/channel",
        "@a/audio/x/channel",
        "@a/console/channel",
        "channel",
    ] {
        add(r);
        add(&r.replace("channel", "buss"));
    }
    let labels: Vec<_> = console_systems(&v).into_iter().map(|s| s.label).collect();
    assert_eq!(labels, ["Beta", "alpha", "console-6", "zeta"]);
}

// 279: a vocabulary of 100,000 keys is listed in one pass.
#[test]
fn a_vocabulary_of_a_hundred_thousand_keys_is_listed_fast() {
    let (_, mut v) = world(&[]);
    for n in 0..100_000 {
        v.modules.insert(
            format!("@k7-core/audio/m{n}"),
            LoadedModule::bare(ModuleKind::Plugin, "audio", "m"),
        );
    }
    let systems = lanes_support::fast("100,000 keys", 5, || console_systems(&v));
    assert_eq!(systems.len(), 1);
}

// 279: both halves resolved; a number param only; the path the preset or ""; the engine's wire form.
#[test]
fn the_halves_are_plugins_with_number_params_only() {
    let (src, v) = world(&[]);
    let c = console_of(&src, &mem_world_roots(), &v, Some(SYS)).expect("console");
    assert!(c.fault.is_none() && c.findings.is_empty(), "{:?}", c.findings);
    let (ch, bus) = (c.channel.expect("channel"), c.buss.expect("buss"));
    assert_eq!((ch.kind.as_str(), ch.name.as_str()), ("Console 6 Channel", CONSOLE_ID));
    assert_eq!(
        ch.params.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(),
        [("Drive", 0.5), ("Trim", -3.0)]
    );
    assert_eq!(
        (ch.path.as_str(), ch.preset_save.as_str(), ch.role.as_str(), ch.enabled),
        ("", "", "", true)
    );
    assert_eq!(bus.kind, "Console 6 Buss");
    // The keys a `PluginDoc` carries, `presetSave` camel-cased.
    let json = serde_json::to_value(&ch).expect("json");
    let mut keys: Vec<_> = json.as_object().expect("an object").keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["enabled", "id", "name", "params", "path", "presetSave", "role", "type"]
    );
    // 2h clones a half per track under its own id.
    let g: HalfPlugin = ch.with_id("gtr");
    assert_eq!(
        (g.id.as_str(), g.name.as_str(), g.kind.as_str()),
        ("@console:gtr", "@console:gtr", "Console 6 Channel")
    );
    assert_eq!(bus.with_id("master").id, "@console:master");
}

// console.test.ts "changes nothing without a console": no system, nothing returned; an empty one too.
#[test]
fn no_system_gives_no_halves() {
    let (src, v) = world(&[]);
    for s in [None, Some("")] {
        let c = console_of(&src, &mem_world_roots(), &v, s).expect("console");
        assert!(c.channel.is_none() && c.buss.is_none() && c.fault.is_none());
    }
}

// console.test.ts "stops the compile on a system with a half missing, saying which" (280): one fault, `channel` before `buss`.
#[test]
fn a_system_with_a_half_missing_is_one_fault_naming_it() {
    let (src, v) = world(&[]);
    let c = console_of(&src, &mem_world_roots(), &v, Some("@k7-core/console/broken")).expect("console");
    let f = c.fault.expect("a fault");
    assert_eq!(
        (f.kind, f.key().as_str()),
        (FaultKind::ConsoleAbsent, "console_absent|console|buss")
    );
    assert!(c.channel.is_none() && c.buss.is_none());
    let none = console_of(&src, &mem_world_roots(), &v, Some("@k7-core/console/ghost")).expect("console");
    assert_eq!(none.fault.expect("a fault").key(), "console_absent|console|channel");
    // The resolve's own findings pass through: a half that extends nothing is an unknown extends, not ours to word.
    assert!(
        none.findings
            .iter()
            .all(|f| f.severity == Severity::Warning || f.severity == Severity::Error)
    );
}

// 279: a jsdsp half is the graph host named by the id; a faust one the Faust host; an external format keeps its name.
#[test]
fn engine_plugin_maps_the_authored_form_onto_the_wire_form() {
    let p = |t: &str, n: Option<&str>| Plugin {
        plugin_type: Some(t.to_owned()),
        name: n.map(str::to_owned),
        ..Plugin::default()
    };
    assert_eq!(
        engine_plugin(Some(&p("jsdsp", None)), "id", Some("audio")),
        ("DSP Graph".into(), "id".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("jsdsp", None)), "id", Some("midi")),
        ("MIDI Graph".into(), "id".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("faust", None)), "id", None),
        ("Faust".into(), "id".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("internal", Some("Chorus"))), "id", None),
        ("Chorus".into(), "id".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("vst3", Some("Big"))), "id", None),
        ("vst3".into(), "Big".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("vst3", None)), "id", None),
        ("vst3".into(), "id".into())
    );
    assert_eq!(
        engine_plugin(Some(&p("Legacy", None)), "id", None),
        ("Legacy".into(), "id".into())
    );
    assert_eq!(engine_plugin(None, "id", None), (String::new(), "id".into()));
    let (src, v) = world(&[
        (
            "lib/k7-core/modules/console/dsp/channel/module.k7x",
            "plugin({ type: \"jsdsp\" })\n".into(),
        ),
        (
            "lib/k7-core/modules/console/dsp/buss/module.k7x",
            "plugin({ type: \"internal\", name: \"B\" })\n".into(),
        ),
    ]);
    let c = console_of(&src, &mem_world_roots(), &v, Some("@k7-core/console/dsp")).expect("console");
    let ch = c.channel.expect("channel");
    assert_eq!(ch.kind, "DSP Graph");
    assert_eq!(ch.with_id("gtr").name, "@console:gtr");
}

// console.test.ts "puts the channel right after the strip..." through the helpers: the slot, and who takes a channel.
#[test]
fn the_channel_goes_after_the_last_strip_and_some_tracks_take_none() {
    assert_eq!(channel_slot(&["Amp", "Mixer", "Meter"]), 2);
    assert_eq!(channel_slot(&[]), 0);
    assert_eq!(channel_slot(&["Amp"]), 1);
    assert_eq!(channel_slot(&["Mixer", "Amp", "Mixer", "Meter"]), 3);
    assert!(takes_channel(&[], "regular") && takes_channel(&[], ""));
    assert!(!takes_channel(&[3, 4], "regular") && !takes_channel(&[], "effect"));
    assert_eq!(CONSOLE_HALVES, ["channel", "buss"]);
}

// A half whose module has 100,000 params is built once; the clone per track is 2h's.
#[test]
fn a_half_of_a_hundred_thousand_params_is_built_once() {
    let params: String = (0..100_000).map(|n| format!("p{n}: {n},")).collect();
    let (src, v) = world(&[
        (
            "lib/k7-core/modules/console/wide/channel/module.k7x",
            format!("plugin({{ type: \"internal\", name: \"W\" }})\nparams({{ {params} }})\n"),
        ),
        (
            "lib/k7-core/modules/console/wide/buss/module.k7x",
            "plugin({ type: \"internal\", name: \"B\" })\n".into(),
        ),
    ]);
    let c = lanes_support::fast("a half of 100,000 params", 5, || {
        console_of(&src, &mem_world_roots(), &v, Some("@k7-core/console/wide")).expect("console")
    });
    assert_eq!(c.channel.expect("channel").params.len(), 100_000);
}
