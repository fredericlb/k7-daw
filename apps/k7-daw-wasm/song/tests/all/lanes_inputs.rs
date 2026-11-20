// lanes_inputs.rs — `load_inputs` (decisions 245 to 249, 265): the project read once, what stops a compile, and a refused family.
// A reader would get wrong that only `Missing` is an `Err`: a refused or unreadable file is a finding and the rest of the project reads.

use crate::lanes_support;
use crate::support;

use lanes_support::{Failing, inputs, lane};
use song::lanes::inputs::{family_or_warning, load_inputs};
use song::lanes::rig::Rig;
use song::modules::Absent;
use song::{Cache, Clip, Severity, SourceError, Timeline, read_family};
use support::{Tmp, disk, mem, mem_world_roots, roots};

const GRAPH: &str = "modules({ drums: [\"amp\"], amp: [\"OUT\"] })\n";

// 246: the order: nodes, lanes, derived chains, the three families, the paths.
#[test]
fn a_project_is_read_whole_and_its_chains_derived_from_its_lanes() {
    let i = inputs(&[
        ("project.k7x", "project({ name: \"p\", tempo: 100 })\n"),
        ("nodes/drums.k7x", &lane("events([ref(\"kick\").start(\"1:1\")])\n")),
        ("nodes/amp.k7x", "extends(\"@k7-core/audio/amp\")\n"),
        ("graph.k7x", GRAPH),
        ("clips/kick.k7x", "source(\"clips/kick.wav\")\n"),
        ("sequences/riff.k7x", "notes([t(0).d(1).k(60)])\n"),
        (
            "timelines/verse.k7x",
            "length(\"2:0\")\nlanes([lane(\"drums\").events([ref(\"kick\").start(\"1:1\")])])\n",
        ),
        ("lib/k7-core/modules/audio/amp/module.k7x", &support::plugin_text("Amp")),
    ]);
    assert!(i.stops().is_empty(), "{:?}", i.stops());
    assert_eq!(i.chains.keys().collect::<Vec<_>>(), ["drums"]);
    assert_eq!(i.chains["drums"].use_steps[0].module, "amp");
    assert_eq!(
        (i.clips.values.len(), i.sequences.values.len(), i.timelines.values.len()),
        (1, 1, 1)
    );
    assert_eq!(i.audio_paths["kick"], "/p/ws/proj/clips/kick.wav");
    assert!(i.module("drums").is_some() && i.module("amp").is_some() && i.module("nope").is_none());
    assert_eq!(i.project, "/p/ws/proj");
}

// 245: a module a chain names is resolved once, as an extra, and never twice nor as a lane.
#[test]
fn a_vocabulary_module_a_chain_names_is_resolved_once_apart() {
    let i = inputs(&[
        ("nodes/drums.k7x", &lane("")),
        (
            "graph.k7x",
            "modules({ drums: [\"@k7-core/audio/amp\"], \"@k7-core/audio/amp\": [\"OUT\"] })\n",
        ),
        ("lib/k7-core/modules/audio/amp/module.k7x", &support::plugin_text("Amp")),
    ]);
    assert!(i.extra.modules.contains_key("@k7-core/audio/amp"));
    assert!(!i.local.modules.contains_key("@k7-core/audio/amp"));
    assert_eq!(i.local.modules.len(), 1);
    assert!(i.module("@k7-core/audio/amp").is_some());
}

// 249: what stops a compile: the errors of the head, the routing, the nodes, a resolve and `timelines/`; a clip's never.
#[test]
fn only_the_named_errors_stop_a_compile() {
    let clean = inputs(&[("nodes/a.k7x", &lane(""))]);
    assert!(clean.stops().is_empty());
    let broken_clip = inputs(&[("nodes/a.k7x", &lane("")), ("clips/bad.k7x", "source(\n")]);
    assert!(broken_clip.stops().is_empty());
    assert_eq!(broken_clip.clips.findings.len(), 1);
    let broken_seq = inputs(&[("sequences/bad.k7x", "notes(\n")]);
    assert!(broken_seq.stops().is_empty());
    let broken_timeline = inputs(&[("timelines/bad.k7x", "lanes(\n")]);
    assert_eq!(
        broken_timeline
            .stops()
            .iter()
            .map(|f| f.file.as_str())
            .collect::<Vec<_>>(),
        ["timelines/bad.k7x"]
    );
    let broken_node = inputs(&[("nodes/bad.k7x", "extends(\n")]);
    assert_eq!(broken_node.stops().len(), 1);
    let broken_head = inputs(&[("project.k7x", "project(\n")]);
    assert_eq!(broken_head.stops().len(), 1);
    let broken_graph = inputs(&[("graph.k7x", "modules(\n")]);
    assert_eq!(broken_graph.stops()[0].file, "graph.k7x");
}

// 249: a warning never stops a compile, and is kept.
#[test]
fn a_warning_is_kept_and_stops_nothing() {
    let i = inputs(&[
        ("nodes/a.k7x", &lane("")),
        ("lib/k7-core/modules/audio/bad/module.k7x", "plugin(\n"),
    ]);
    assert!(i.stops().is_empty(), "{:?}", i.stops());
    assert!(
        i.warnings.iter().any(|f| f.file.ends_with("bad/module.k7x")),
        "{:?}",
        i.warnings
    );
    assert!(i.warnings.iter().all(|f| f.severity == Severity::Warning));
}

// 247: a family folder the Source refuses is one warning named by its folder; one file refused costs that file alone.
#[test]
fn a_family_the_source_refuses_is_a_warning_and_an_empty_family() {
    let t = Tmp::new("lanes-outside");
    t.write("ws/proj/nodes/a.k7x", &lane(""));
    t.mkdir("out");
    t.write("out/kick.k7x", "source(\"x.wav\")\n");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(t.0.join("out"), t.0.join("ws/proj/clips")).is_ok();
    #[cfg(not(unix))]
    let linked = false;
    if !linked {
        return support::skip("a_family_the_source_refuses", "no links");
    }
    t.age();
    let r = roots(&t);
    let src = disk(&r);
    let i = load_inputs(&src, &mut Cache::new(), &r, Rig::default()).expect("inputs");
    assert!(i.clips.values.is_empty());
    let files: Vec<_> = i.clips.findings.iter().map(|f| (f.file.as_str(), f.severity)).collect();
    assert_eq!(files, [("clips", Severity::Warning)]);
    assert!(i.stops().is_empty());
    assert!(read_family::<Clip>(&src, &mut Cache::new(), &i.project).is_err());
    let again = family_or_warning::<Timeline>(&src, &mut Cache::new(), &i.project).expect("family");
    assert!(again.values.is_empty() && again.findings.is_empty());
}

// 248: a file that answers Io is an ERROR for that file and its siblings read.
#[test]
fn an_unreadable_file_costs_that_file_alone_and_counts_as_broken() {
    let files = [
        ("ws/proj/clips/a.k7x", "source(\"a.wav\")\n"),
        ("ws/proj/clips/b.k7x", "source(\"b.wav\")\n"),
        ("ws/proj/timelines/t.k7x", "lanes([])\n"),
    ];
    let m = mem(&files);
    let failing = Failing {
        inner: &m,
        path: "/p/ws/proj/clips/a.k7x".into(),
    };
    let fam = read_family::<Clip>(&failing, &mut Cache::new(), "/p/ws/proj").expect("reads");
    assert_eq!(fam.values.iter().map(|(i, _)| i.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(fam.findings.len(), 1);
    assert_eq!(
        (fam.findings[0].file.as_str(), fam.findings[0].severity),
        ("clips/a.k7x", Severity::Error)
    );
    let failing = Failing {
        inner: &m,
        path: "/p/ws/proj/timelines/t.k7x".into(),
    };
    let i = load_inputs(&failing, &mut Cache::new(), &mem_world_roots(), Rig::default()).expect("inputs");
    assert_eq!(
        i.stops().iter().map(|f| f.file.as_str()).collect::<Vec<_>>(),
        ["timelines/t.k7x"]
    );
    let missing = load_inputs(&mem(&[]), &mut Cache::new(), &mem_world_roots(), Rig::default());
    assert!(!matches!(missing, Err(SourceError::Io(_))));
}

// 265: the vocabulary's broken files are sorted, and `absent` finds the ref's run by binary search.
#[test]
fn absent_answers_from_a_sorted_list_of_broken_files() {
    let i = inputs(&[
        ("lib/k7-core/modules/audio/zed/module.k7x", "plugin(\n"),
        ("lib/k7-core/modules/audio/amp/module.k7x", "plugin(\n"),
        ("lib/k7-core/modules/audio/amp2/module.k7x", "plugin(\n"),
    ]);
    let v = &i.vocabulary;
    assert!(v.broken.windows(2).all(|w| w[0] <= w[1]), "{:?}", v.broken);
    assert!(
        matches!(v.absent("@k7-core/audio/amp"), Absent::Broken { file } if file == "@k7-core/audio/amp/module.k7x")
    );
    assert!(matches!(v.absent("@k7-core/audio/zed"), Absent::Broken { .. }));
    assert_eq!(v.absent("@k7-core/audio/am"), Absent::Unknown);
    assert_eq!(v.absent("@k7-core/audio/nope"), Absent::Unknown);
}
