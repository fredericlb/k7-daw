// graph_validate.rs — validate_issues over the 33 codes, and the sentences around them (404 to 412).
// A reader would get wrong that a port mismatch refuses the graph and never reaches here: it is a wiring fault (405).

use crate::graph_support;

use graph_support::{OUT_AMP, keys, lane, refused, revalidated, sentences, with_graph};
use song::graph::WarningCode;

const AMP: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\ninputs([{ name: \"in\", type: \"audio\" }])\n";
const IN: &str = "io(\"in\")\nchannel([1])\n";

fn lane_a() -> String {
    lane("")
}

fn has(files: &[(&str, &str)], key: &str) -> bool {
    keys(files).iter().any(|k| k == key)
}

// graph.test.ts "passes a clean chain routed to an io sink with no warnings", ports.test.ts "opens clean".
#[test]
fn a_clean_chain_to_an_io_sink_says_nothing() {
    let a = lane_a();
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("graph.k7x", "modules({ a: [\"OUT\"] })\n"),
    ];
    assert_eq!(sentences(&files), Vec::<String>::new());
}

// graph.test.ts "gives each warning a code and the names in it, its sentence built from them".
#[test]
fn a_warning_has_a_code_names_and_its_sentence() {
    let files = [("graph.k7x", "modules({ \"@in\": [\"OUT\"] })\n")];
    with_graph(&files, |g| {
        let i = g
            .issues
            .iter()
            .find(|i| i.code == WarningCode::ChainFromUnknown)
            .expect("issue");
        assert_eq!(i.key(), "chainFromUnknown|chain=@in|from=@in");
        assert_eq!(i.text, "chain '@in': from '@in' is not a track or chain");
        assert_eq!(g.warnings.first().map(String::as_str), Some(i.text.as_str()));
    });
}

// graph.test.ts "flags a step whose module does not exist"; missingModule.test.ts "is reported as a graph warning".
#[test]
fn a_step_naming_no_module_is_a_warning_and_still_a_chain() {
    let files = [("graph.k7x", "modules({ ghost: [\"OUT\"] })\n")];
    assert!(has(&files, "chainStepUnknown|chain=ghost|module=ghost|hint="));
    let s = sentences(&files);
    assert!(
        s.iter()
            .any(|w| w == "chain 'ghost': step module 'ghost' is not a module or module node")
    );
}

// namespacedStep.test.ts "is told which character to change": a ref spelled with a slash where a node takes a colon.
#[test]
fn a_ref_with_a_slash_where_a_node_takes_a_colon_is_told_which_to_change() {
    let files = [
        ("lib/k7-core/modules/io/IN/module.k7x", "io(\"in\")\n"),
        ("lib/k7-core/modules/io/IN/in-2.k7x", "channel([2])\n"),
        ("graph.k7x", "modules({ \"@k7-core/io/IN/in-2\": [\"OUT\"] })\n"),
    ];
    assert!(
        sentences(&files)
            .iter()
            .any(|w| w.contains("did you mean '@k7-core/io/IN:in-2'"))
    );
}

// namespacedStep.test.ts "resolves, rather than being reported" and "works in the sink position too".
#[test]
fn a_ref_in_a_step_and_in_a_sink_position_resolves() {
    let a = lane_a();
    let files = [
        (
            "lib/k7-core/modules/audio/shared-delay/module.k7x",
            "plugin({ type: \"internal\", name: \"delay\" })\n",
        ),
        ("lib/k7-core/modules/io/desk/module.k7x", "io(\"out\")\n"),
        ("nodes/a.k7x", a.as_str()),
        (
            "graph.k7x",
            "modules({ a: [\"@k7-core/audio/shared-delay\"], \"@k7-core/audio/shared-delay\": [\"@k7-core/io/desk\"] })\n",
        ),
    ];
    assert_eq!(sentences(&files), Vec::<String>::new());
    with_graph(&files, |g| {
        assert!(g.modules.contains_key("@k7-core/audio/shared-delay"));
        assert_eq!(g.chains["a"].out[0].to, "@k7-core/io/desk");
    });
}

// ports.test.ts "accepts both inputs fed" and "follows a side input through to the sink, and back for a cycle".
#[test]
fn both_inputs_fed_says_nothing_and_a_side_input_reaches_the_sink() {
    let xf = "plugin({ type: \"jsdsp\" })\ninputs([{ name: \"a\", type: \"audio\" }, { name: \"b\", type: \"audio\", mandatory: false }])\noutputs([{ name: \"out\", type: \"audio\" }])\n";
    let a = lane("events([ref(\"k\").start(\"1:1\")])\n");
    let files = [
        ("clips/k.k7x", "source(\"clips/k.wav\")\n"),
        ("nodes/crossfade.k7x", xf),
        ("nodes/a.k7x", a.as_str()),
        ("nodes/b.k7x", a.as_str()),
        (
            "graph.k7x",
            "modules({ a: [\"crossfade\"], b: [\"crossfade#b\"], crossfade: [\"OUT\"] })\n",
        ),
    ];
    assert_eq!(sentences(&files), Vec::<String>::new());
    // b lands INSIDE a's run, so it reaches OUT through it and there is no cycle.
    with_graph(&files, |g| {
        assert!(g.chains.contains_key("b") && !g.chains.contains_key("crossfade"))
    });
}

// ports.test.ts "refuses more sources than the module has ports": arity refuses the graph, it does not warn.
#[test]
fn more_sources_than_ports_refuse_the_graph() {
    let xf = "plugin({ type: \"jsdsp\" })\ninputs([{ name: \"a\", type: \"audio\" }])\n";
    let a = lane_a();
    let files = [
        ("nodes/crossfade.k7x", xf),
        ("nodes/a.k7x", a.as_str()),
        ("nodes/b.k7x", a.as_str()),
        (
            "graph.k7x",
            "modules({ a: [\"crossfade\"], b: [\"crossfade#a\"], crossfade: [\"OUT\"] })\n",
        ),
    ];
    let r = refused(&files);
    assert!(
        r.faults.iter().any(|f| f.key().starts_with("too_many|chain:a:1|")),
        "{:?}",
        r.faults
    );
}

// docModule.test.ts "does not make the project complain": a note ends a run and is a sink by shape.
#[test]
fn a_note_is_a_sink_and_makes_no_warning() {
    let a = lane_a();
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("nodes/note.k7x", "extends(\"@meta/doc\")\ncontent(\"hi\")\n"),
        ("graph.k7x", "modules({ a: [\"note\"] })\n"),
    ];
    assert_eq!(sentences(&files), Vec::<String>::new());
}

// fileInput.test.ts "is not somewhere a chain can route TO"; midiIn.test.ts "is refused as a destination".
#[test]
fn a_source_io_is_no_destination_and_a_midi_device_is_named() {
    let a = lane_a();
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("nodes/f.k7x", "io(\"file\")\nsource(\"x.wav\")\n"),
        ("graph.k7x", "modules({ a: [\"f\"] })\n"),
    ];
    assert!(has(&files, "outUnknown|chain=a|out=f"));
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("nodes/m.k7x", "io(\"midi\")\n"),
        ("graph.k7x", "modules({ a: [\"m\"] })\n"),
    ];
    assert!(has(&files, "outIsMidiDevice|chain=a|out=m"));
    // midiIn.test.ts "is a legal cable": a MIDI device feeding a lane is no warning.
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("nodes/m.k7x", "io(\"midi\")\n"),
        ("graph.k7x", "modules({ m: [\"a\"], a: [\"OUT\"] })\n"),
    ];
    assert!(keys(&files).iter().all(|k| !k.starts_with("outIsTrack")));
}

// linkLevels.test.ts "is drawn on the graph's out" and "on a link to MIX is not played".
#[test]
fn a_link_level_is_on_the_out_and_one_not_played_is_said() {
    let a = lane_a();
    let chain = "start(true)\nsteps([\n  id(\"a\").track(\"a\"),\n  id(\"amp\").plugin({ type: \"internal\", name: \"Chorus\" }),\n  to([chain(\"OUT\").level(0.5)]),\n])\n";
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("tracks/a.k7x", a.as_str()),
        ("chains/main.k7x", chain),
    ];
    with_graph(&files, |g| {
        let outs: Vec<f64> = g.chains.values().flat_map(|c| c.out.iter().map(|o| o.gain)).collect();
        assert!(outs.contains(&0.5), "{outs:?}");
        assert!(
            g.warnings
                .iter()
                .any(|w| w.contains("the level 0.5 on its link to 'OUT' is not played"))
        );
    });
}

/// The keys of a project built from `graph` and the files given, `a` being a lane and `amp` a plugin beside them.
fn project(graph: &str, extra: &[(&str, &str)]) -> Vec<String> {
    let a = lane_a();
    let mut files: Vec<(&str, &str)> = vec![
        ("nodes/a.k7x", a.as_str()),
        ("nodes/amp.k7x", AMP),
        ("graph.k7x", graph),
    ];
    files.extend_from_slice(extra);
    keys(&files)
}

fn expect(graph: &str, extra: &[(&str, &str)], key: &str) {
    let got = project(graph, extra);
    assert!(got.iter().any(|k| k == key), "{key} not in {got:?}");
}

// One case per code a file can make: a project the TypeScript's validate warns about, with the exact key.
#[test]
fn every_code_a_file_can_make_is_raised_with_its_names() {
    let m = lane("events([ref(\"k\").start(\"1:1\"), ref(\"r\").start(\"2:1\")])\n");
    expect(
        "modules({ m: [\"amp\"], amp: [\"OUT\"] })\n",
        &[
            ("clips/k.k7x", "source(\"clips/k.wav\")\n"),
            ("sequences/r.k7x", "notes([t(0).d(1).k(60)])\n"),
            ("nodes/m.k7x", m.as_str()),
        ],
        "chainMixedLane|chain=m|track=m|module=amp|wants=audio",
    );
    expect(
        "modules({ \"@in\": [\"OUT\"] })\n",
        &[],
        "chainFromUnknown|chain=@in|from=@in",
    );
    expect(
        "modules({ a: [\"nope\"], nope: [\"OUT\"] })\n",
        &[("nodes/nope.k7x", "extends(\"no-such-ref\")\n")],
        "chainExtendsUnknown|chain=a|module=nope|base=no-such-ref",
    );
    expect(
        "modules({ a: [\"wipe\"], wipe: [\"OUT\"] })\n",
        &[
            (
                "lib/k7-core/modules/video_transitions/wipe/module.k7x",
                "type(\"video_transition\")\n",
            ),
            ("nodes/wipe.k7x", "extends(\"@k7-core/video_transitions/wipe\")\n"),
        ],
        "chainTransitionWired|chain=a|module=wipe",
    );
    let bad = |graph: &str, extra: &[(&str, &str)], key: &str| expect(graph, extra, key);
    bad(
        "modules({ a: [\"mic\"] })\n",
        &[("nodes/mic.k7x", IN)],
        "outIsInput|chain=a|out=mic",
    );
    bad(
        "modules({ a: [\"cam\"] })\n",
        &[("nodes/cam.k7x", "io(\"camera\")\n")],
        "outIsCamera|chain=a|out=cam",
    );
    bad(
        "modules({ a: [\"dev\"] })\n",
        &[("nodes/dev.k7x", "io(\"midi\")\n")],
        "outIsMidiDevice|chain=a|out=dev",
    );
    bad(
        "modules({ a: [\"amp\"], amp: [\"pic\"] })\n",
        &[("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n")],
        "outIsPictureLane|chain=a|out=pic",
    );
    bad(
        "modules({ a: [\"amp\"], amp: [\"b\"] })\n",
        &[("nodes/b.k7x", "extends(\"@meta/track\")\n")],
        "outIsTrack|chain=a|out=b",
    );
    bad("modules({ a: [\"ZZZ\"] })\n", &[], "outUnknown|chain=a|out=ZZZ");
    bad(
        "modules({ a: [\"amp\"], \"amp#nope\": [\"OUT\"] })\n",
        &[],
        "outBadPort|chain=a|out=OUT|source=amp|port=nope|outputs=",
    );
    bad(
        "modules({ a: [\"two\"], \"two#nope\": [\"OUT\"] })\n",
        &[(
            "nodes/two.k7x",
            "plugin({ type: \"internal\", name: \"X\" })\noutputs([{ name: \"out\", type: \"audio\" }, { name: \"aux\", type: \"audio\" }])\n",
        )],
        "outBadPort|chain=a|out=OUT|source=two|port=nope|outputs=out, aux",
    );
    bad(
        "modules({ a: [\"RENDER#bogus\"] })\n",
        &[],
        "outRenderPort|chain=a|out=RENDER#bogus",
    );
    bad(
        "modules({ a: [\"RENDER#audio\"] })\n",
        &[],
        "outRenderSoundNotMix|chain=a|out=RENDER#audio",
    );
}

// The codes about lanes, inputs, groups and the reach of a chain.
#[test]
fn the_lane_input_group_and_reach_codes_are_raised_with_their_names() {
    expect(
        "modules({ e: [\"OUT\"] })\n",
        &[(
            "nodes/e.k7x",
            "extends(\"@meta/track\")\nevents([ref(\"nope\").start(\"1:1\")])\n",
        )],
        "trackClipMissing|track=e|ref=nope",
    );
    expect(
        "modules({ a: [\"OUT\"] })\n",
        &[(
            "nodes/n.k7x",
            "plugin({ type: \"internal\", name: \"Amp\" })\nparams({ model: \"room.nam\" })\n",
        )],
        "moduleAssetMissing|module=n|param=model|ref=room.nam",
    );
    expect(
        "modules({ mic: [\"OUT\"] })\n",
        &[("nodes/mic.k7x", "io(\"in\")\n")],
        "inputNoChannel|input=mic",
    );
    let g = lane("disabled(true)\n");
    expect(
        "modules({ mic: [\"d\"] })\n",
        &[("nodes/mic.k7x", IN), ("nodes/d.k7x", g.as_str())],
        "inputTrackDisabled|input=mic|track=d",
    );
    expect(
        "modules({ pic: [\"mic\"], mic: [\"OUT\"] })\n",
        &[
            ("nodes/mic.k7x", IN),
            ("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n"),
        ],
        "inputTrackMissing|input=mic|track=pic",
    );
    expect(
        "modules({ mic: [\"grp\"] })\n",
        &[
            ("nodes/mic.k7x", IN),
            ("nodes/grp.k7x", "extends(\"@meta/group-track\")\ntracks([\"a\"])\n"),
        ],
        "inputTrackGroup|input=mic|track=grp",
    );
    expect(
        "modules({ mic: [\"kit\"] })\n",
        &[
            ("nodes/mic.k7x", IN),
            (
                "nodes/kit.k7x",
                "extends(\"@meta/strudel-track\")\nstrudel(\"algo/x.strudel\")\n",
            ),
        ],
        "inputTrackStrudel|input=mic|track=kit",
    );
    expect(
        "modules({ m1: [\"a\"], m2: [\"a\"] })\n",
        &[("nodes/m1.k7x", IN), ("nodes/m2.k7x", IN)],
        "trackTwoInputs|track=a|first=m1|second=m2",
    );
    expect(
        "modules({ a: [\"amp\"] })\n",
        &[(
            "nodes/chan.k7x",
            "plugin({ type: \"internal\", name: \"Amp\" })\nchannel([2])\n",
        )],
        "moduleChannelNotInput|module=chan",
    );
    expect(
        "modules({})\n",
        &[(
            "nodes/g.k7x",
            "extends(\"@meta/group-track\")\ntracks([\"g\", \"ghost\"])\n",
        )],
        "groupContainsItself|group=g",
    );
    expect(
        "modules({})\n",
        &[(
            "nodes/g.k7x",
            "extends(\"@meta/group-track\")\ntracks([\"g\", \"ghost\"])\n",
        )],
        "groupTrackMissing|group=g|track=ghost",
    );
    expect(
        "modules({})\n",
        &[
            ("nodes/g1.k7x", "extends(\"@meta/group-track\")\ntracks([\"a\"])\n"),
            ("nodes/g2.k7x", "extends(\"@meta/group-track\")\ntracks([\"a\"])\n"),
        ],
        "trackTwoGroups|track=a|first=g1|second=g2",
    );
    expect(
        "modules({ a: [\"amp\"], amp: [\"b\"] })\n",
        &[("nodes/b.k7x", "extends(\"@meta/track\")\n")],
        "chainNoOut|chain=a",
    );
    expect(
        "modules({ x: [\"y\"], y: [\"x\"] })\n",
        &[("nodes/x.k7x", OUT_AMP), ("nodes/y.k7x", OUT_AMP)],
        "chainCycle|chain=x",
    );
}

// The codes no file can make (a negative level, a chain with no out, an assembly) are raised on a graph changed in memory.
#[test]
fn the_codes_no_file_can_make_are_raised_on_a_changed_graph() {
    let a = lane_a();
    let files = [
        ("nodes/a.k7x", a.as_str()),
        ("graph.k7x", "modules({ a: [\"OUT\"] })\n"),
    ];
    let got = revalidated(&files, |g| {
        g.chains[0].out[0].gain = -1.0;
    });
    assert!(got.contains(&"outNegativeGain|chain=a|out=OUT".to_owned()), "{got:?}");
    let got = revalidated(&files, |g| g.chains[0].out.clear());
    assert!(got.contains(&"chainDeadEnd|chain=a".to_owned()), "{got:?}");
    let got = revalidated(&files, |g| {
        use song::assemblies::Assembly;
        let box_ = |members: &[(&str, &str)]| Assembly {
            uses: "@k7-core/x".into(),
            label: "x".into(),
            members: members.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: indexmap::IndexMap::new(),
            missing: None,
        };
        g.assemblies.insert("a".into(), box_(&[("m", "ghost")]));
        g.assemblies.insert("second".into(), box_(&[("m", "ghost")]));
    });
    for key in [
        "assemblyNameTaken|assembly=a",
        "assemblyMemberMissing|assembly=a|member=m|node=ghost",
        "nodeTwoAssemblies|node=ghost|first=a|second=second",
    ] {
        assert!(got.contains(&key.to_owned()), "{key} not in {got:?}");
    }
}
