// views_hostile.rs — the A2 views on hostile projects: the I/O ports and the completion vocabulary answer fast, and say what they cut.
// A reader would get wrong that a case builds its project by hand: the 2a memory source scans everything held for each path it lacks.

use crate::support;
use crate::views_support;
use crate::views_world;

use indexmap::IndexMap;
use song::files::{Clip, Sequence};
use song::lanes::inputs::Inputs;
use song::modules::{Input, IoKind, LoadedModule, ModuleKind};
use song::views::BUDGET;
use song::views::catalogue::{base_modules, module_catalogue};
use song::views::hints::{HintsAnswer, hints_of};
use song::views::io_ports::{IoDir, IoPorts, io_ports};
use song::{Cache, Kind, Severity};
use std::sync::Arc;
use views_support::fast;
use views_world::{Fake, bare, decoded, entry, plugin_module};

fn io_node(io: IoKind, channel: Option<Input>) -> LoadedModule {
    let mut m = LoadedModule::bare(ModuleKind::Io, "io", "n");
    m.authored.io = Some(io);
    m.authored.channel = channel;
    m
}

fn with_nodes(nodes: IndexMap<String, LoadedModule>) -> Inputs {
    let mut i = bare(IndexMap::new());
    i.nodes.nodes = nodes;
    i
}

fn assert_capped_by_budget(spent: usize, capped: bool, what: &str) {
    assert!(capped, "{what}: not capped at {spent}");
    assert!(spent > BUDGET && spent < BUDGET * 2, "{what}: spent {spent}");
}

#[test]
fn a_hundred_thousand_inputs_then_a_hundred_thousand_outputs() {
    // hostile table (io_ports): 100,000 local nodes of `kind: io`, every port `in` then every port `out`, inputs first (622, 624).
    let nodes: IndexMap<String, LoadedModule> = (0..100_000)
        .map(|i| {
            let io = if i % 2 == 0 { IoKind::In } else { IoKind::Out };
            (
                format!("n{i}"),
                io_node(io, Some(Input::Channels(vec![1 + (i % 8) as u64]))),
            )
        })
        .collect();
    let inputs = with_nodes(nodes);
    let p: IoPorts = fast("100,000 ports", || io_ports(&inputs));
    println!("100,000 ports spent {} units", p.spent);
    assert_eq!(p.ports.len(), 100_000);
    assert!(!p.capped && p.findings.is_empty() && p.spent < BUDGET);
    let first_out = p.ports.iter().position(|x| x.dir == IoDir::Out).expect("an output");
    assert_eq!(first_out, 50_000);
    assert!(p.ports[first_out..].iter().all(|x| x.dir == IoDir::Out));
}

#[test]
fn every_port_of_one_direction_is_listed_in_ref_order() {
    // hostile table (io_ports): all `in`, then all `out`; the order is by `ref` within a direction (622).
    for io in [IoKind::In, IoKind::Out] {
        let nodes: IndexMap<String, LoadedModule> = (0..5_000).map(|i| (format!("n{i}"), io_node(io, None))).collect();
        let p = io_ports(&with_nodes(nodes));
        let refs: Vec<&str> = p.ports.iter().map(|x| x.ref_.as_str()).collect();
        let mut sorted = refs.clone();
        sorted.sort_unstable();
        assert_eq!((refs.len(), refs == sorted), (5_000, true));
        // A label absent: no channel, no label, no unknown label.
        assert!(
            p.ports
                .iter()
                .all(|x| x.channels.is_empty() && x.label.is_none() && x.unknown_label.is_none())
        );
    }
}

#[test]
fn a_channel_list_of_a_hundred_thousand_numbers_is_cut_at_the_rigs_sixty_four() {
    // hostile table (io_ports): 100,000 channels on one; only a u32 above 0 counts, 64 at most (624).
    let list: Vec<u64> = (0..100_000).map(|i| if i < 10 { 0 } else { i + 1 }).collect();
    let nodes: IndexMap<String, LoadedModule> = (0..20)
        .map(|i| {
            (
                format!("n{i}"),
                io_node(IoKind::In, Some(Input::Channels(list.clone()))),
            )
        })
        .collect();
    let p = fast("20 wide ports", || io_ports(&with_nodes(nodes)));
    assert!(p.ports.iter().all(|x| x.channels.len() == 64 && x.channels[0] == 11));
    // A list of zeros and a number past u32 name nothing.
    let none = IndexMap::from([(
        "z".to_owned(),
        io_node(IoKind::In, Some(Input::Channels(vec![0, 0, u64::MAX]))),
    )]);
    assert!(io_ports(&with_nodes(none)).ports[0].channels.is_empty());
}

#[test]
fn a_port_named_with_a_megabyte_spends_its_weight_and_the_rest_is_cut() {
    // hostile table (io_ports): a port name of 1 MB; ref plus name weigh 2 MB / 64 units each, so the budget ends the list (624).
    let long = "x".repeat(1 << 20);
    let nodes: IndexMap<String, LoadedModule> = (0..40)
        .map(|i| (format!("{i}{long}"), io_node(IoKind::In, None)))
        .collect();
    let p = fast("40 ports of 1 MB", || io_ports(&with_nodes(nodes)));
    assert_capped_by_budget(p.spent, p.capped, "ports of 1 MB");
    assert!(p.ports.len() < 40 && p.ports.len() > 25, "{}", p.ports.len());
    assert_eq!(
        (p.findings.len(), p.findings[0].file.as_str(), p.findings[0].severity),
        (1, "project.k7x", Severity::Warning)
    );
}

fn base_ref(i: usize) -> String {
    format!("@lib/audio/m{i}")
}

fn hints(inputs: &Inputs, source: &dyn song::Source) -> HintsAnswer {
    hints_of(
        source,
        &mut Cache::new(),
        &song::Roots {
            project: inputs.project.clone(),
            ..song::Roots::default()
        },
        inputs,
    )
    .expect("hints")
}

#[test]
fn a_hundred_thousand_refs_of_one_namespace_and_a_hundred_thousand_nodes_of_one_module() {
    // hostile table (module_catalogue, base_modules): 100,000 refs of one namespace, the vocabulary built by hand (the 2a memory source is quadratic), nodes counted in one map.
    let mut vocab: IndexMap<String, LoadedModule> = (0..100_000)
        .map(|i| (base_ref(i), plugin_module("audio", &format!("m{i}"), "reverb")))
        .collect();
    for i in 0..100_000 {
        let mut n = LoadedModule::bare(ModuleKind::Plugin, "audio", &format!("n{i}"));
        n.node = Some(format!("n{i}"));
        vocab.insert(format!("@lib/audio/m0:n{i}"), n);
    }
    let mut fake = Fake::default();
    for i in 0..100_000 {
        fake.file(&format!("/p/lib/modules/audio/m{i}/module.k7x"));
    }
    let roots = song::Roots {
        project: "/p/ws/proj".into(),
        libraries: vec![song::Library {
            name: "lib".into(),
            root: "/p/lib".into(),
            editable: false,
        }],
        ..song::Roots::default()
    };
    let inputs = bare(vocab);
    let (cat, base) = fast("100,000 refs", || {
        (
            module_catalogue(&fake, &roots, &inputs.vocabulary).expect("a catalogue"),
            base_modules(&inputs.vocabulary),
        )
    });
    println!(
        "catalogue of 100,000 refs spent {} units, bases {}",
        cat.spent, base.spent
    );
    assert_eq!((cat.items.len(), cat.capped), (100_000, false));
    assert_eq!(
        cat.items.iter().find(|m| m.ref_ == "@lib/audio/m0").map(|m| m.nodes),
        Some(100_000)
    );
    assert_eq!((base.items.len(), base.capped), (100_000, false));
}

#[test]
fn a_hundred_thousand_nodes_over_a_hundred_thousand_base_refs() {
    // hostile table (hints_of): 100,000 nodes and 100,000 base refs; the answer is cut by the budget, never by the clock (628, 635).
    let vocab: IndexMap<String, LoadedModule> = (0..100_000)
        .map(|i| (base_ref(i), plugin_module("audio", &format!("m{i}"), "reverb")))
        .collect();
    let mut inputs = bare(vocab);
    for i in 0..100_000 {
        let mut n = LoadedModule::bare(ModuleKind::Plugin, "", &format!("n{i}"));
        n.authored.extends = Some(base_ref(i));
        n.home = Some("nodes".to_owned());
        inputs.nodes.nodes.insert(format!("n{i}"), n);
    }
    let a = fast("100,000 nodes and bases", || hints(&inputs, &Fake::default()));
    println!("100,000 + 100,000 spent {} units, capped {}", a.spent, a.capped);
    assert_capped_by_budget(a.spent, a.capped, "100,000 nodes and bases");
    // What was paid for stays: the first lists are whole, the later ones are empty.
    assert_eq!(a.hints.base_modules.len(), 100_000);
    assert!(a.hints.paths.is_empty() && a.hints.media.is_empty());
    assert_eq!((a.findings.len(), a.findings[0].file.as_str()), (1, "project.k7x"));
}

#[test]
fn a_base_with_many_extenders_of_many_params_is_cut_by_the_work() {
    // hostile table (hints_of): one base with thousands of extenders each of 1,000 params is a product; the index walk is charged (628).
    let mut inputs = bare(IndexMap::from([(
        "@lib/audio/m".to_owned(),
        plugin_module("audio", "m", "reverb"),
    )]));
    for i in 0..1_200 {
        let mut n = LoadedModule::bare(ModuleKind::Plugin, "", &format!("n{i}"));
        n.authored.extends = Some("@lib/audio/m".to_owned());
        n.authored.params = Some(
            (0..1_000)
                .map(|k| (format!("p{i}_{k}"), serde_json::from_str("1").expect("v")))
                .collect(),
        );
        inputs.nodes.nodes.insert(format!("n{i}"), n);
    }
    let a = fast("1,200 extenders of 1,000 params", || hints(&inputs, &Fake::default()));
    assert_capped_by_budget(a.spent, a.capped, "extenders");
    assert!(a.hints.params_by_base.is_empty());
}

#[test]
fn ten_thousand_bases_on_a_builtin_of_767_params_are_a_product_the_budget_cuts() {
    // hostile table (hints_of): 10,000 bases on `studio_surge` (767 params); the metadata is charged per entry (628).
    let vocab: IndexMap<String, LoadedModule> = (0..10_000)
        .map(|i| (base_ref(i), plugin_module("audio", &format!("m{i}"), "studio_surge")))
        .collect();
    let inputs = bare(vocab);
    let a = fast("10,000 bases on a builtin", || hints(&inputs, &Fake::default()));
    assert_capped_by_budget(a.spent, a.capped, "builtin bases");
    let meta = a.hints.param_meta_by_base.len();
    assert!((100..1_000).contains(&meta), "{meta}");
    assert!(a.hints.param_meta_by_base.values().all(|m| m.len() == 767));
}

#[test]
fn a_hundred_thousand_clips_and_sequences_are_listed_sorted_in_time() {
    // hostile table (hints_of): 100,000 clips and sequences, one decoded value shared (627, 634).
    let mut inputs = bare(IndexMap::new());
    let clip: Arc<Clip> = decoded("clips/a.k7x", "source(\"a.wav\")\n");
    let seq: Arc<Sequence> = decoded("sequences/a.k7x", "notes([])\n");
    inputs.clips.values = (0..100_000).map(|i| (format!("c{i}"), clip.clone())).collect();
    inputs.sequences.values = (0..100_000).map(|i| (format!("s{i}"), seq.clone())).collect();
    let a = fast("100,000 clips", || hints(&inputs, &Fake::default()));
    println!("100,000 clips and sequences spent {} units", a.spent);
    assert_eq!(a.hints.clips.len(), 100_000);
    let mut sorted = a.hints.clips.clone();
    sorted.sort();
    assert_eq!(a.hints.clips, sorted);
    assert_eq!(a.hints.audio_clips.as_ref().map(Vec::len), Some(100_000));
    // The budget ends the list somewhere after the clips: whatever it cut, it said so.
    assert_eq!(a.capped, !a.findings.is_empty());
}

#[test]
fn many_curves_are_listed_sorted_and_the_files_are_read_through_the_cache() {
    // hostile table (hints_of): 100,000 curves, each file decoded once (627, 634).
    let n = 100_000;
    let names: Vec<String> = (0..n).map(|i| format!("curves/k{i}.k7x")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|p| (p.as_str(), "type(\"lfo\")\n")).collect();
    let src = views_world::world(&files);
    let a = fast("100,000 curves", || hints(&bare(IndexMap::new()), &src));
    let curves = a.hints.curves.as_ref().expect("curves");
    assert_eq!(curves.len(), n);
    let mut sorted = curves.clone();
    sorted.sort();
    assert_eq!(*curves, sorted);
}

#[test]
fn a_folder_of_two_hundred_thousand_audio_names_is_walked_and_weighed() {
    // hostile table (hints_of): a folder of 200,000 audio names; a step and a copy each, grouped under the folder (630).
    let mut fake = Fake::default();
    fake.dir("/p/ws/proj", vec![entry("audio", Kind::Dir)]);
    fake.dir(
        "/p/ws/proj/audio",
        (0..200_000).map(|i| entry(&format!("a{i}.wav"), Kind::File)).collect(),
    );
    let a = fast("200,000 names", || hints(&bare(IndexMap::new()), &fake));
    println!("200,000 names spent {} units", a.spent);
    assert_eq!(a.hints.media["audio"].len(), 200_000);
    assert!(!a.capped, "{}", a.spent);
}

#[test]
fn a_node_named_with_a_megabyte_has_a_link_that_is_paid_for() {
    // hostile table (hints_of): a 1 MB node name in `paths`; the key and the file weigh their bytes (629, 635).
    let long = "y".repeat(1 << 20);
    let mut fake = Fake::default();
    let mut inputs = bare(IndexMap::new());
    for i in 0..40 {
        let name = format!("{i}{long}");
        fake.file(&format!("/p/ws/proj/nodes/{name}.k7x"));
        let mut n = LoadedModule::bare(ModuleKind::Plugin, "", &name);
        n.home = Some("nodes".to_owned());
        inputs.nodes.nodes.insert(name, n);
    }
    let a = fast("40 nodes of 1 MB", || hints(&inputs, &fake));
    assert_capped_by_budget(a.spent, a.capped, "nodes of 1 MB");
    assert_eq!(a.hints.modules.len(), 40);
    assert!((1..40).contains(&a.hints.paths.len()), "{}", a.hints.paths.len());
}

#[test]
fn a_thousand_nodes_over_a_chain_of_seventeen_parents_with_ten_thousand_assets() {
    // hostile table (hints_of): 1,000 nodes each extending a chain of 17 parents holding 10,000 assets; one listing per folder, every copy charged (631).
    let mut fake = Fake::default();
    let mut vocab: IndexMap<String, LoadedModule> = IndexMap::new();
    for p in 0..17 {
        let folder = format!("/p/lib/p{p}");
        fake.dir(
            &format!("{folder}/assets"),
            (0..10_000)
                .map(|i| entry(&format!("f{p}_{i}.nam"), Kind::File))
                .collect(),
        );
        let mut m = plugin_module("audio", &format!("p{p}"), "nam");
        m.folder_abs = Some(folder);
        if p < 16 {
            m.authored.extends = Some(format!("@lib/audio/p{}", p + 1));
        }
        vocab.insert(format!("@lib/audio/p{p}"), m);
    }
    let mut inputs = bare(vocab);
    for i in 0..1_000 {
        let mut n = LoadedModule::bare(ModuleKind::Plugin, "", &format!("n{i}"));
        n.authored.extends = Some("@lib/audio/p0".to_owned());
        n.folder_abs = Some("/p/ws/proj".to_owned());
        inputs.nodes.nodes.insert(format!("n{i}"), n);
    }
    let a = fast("1,000 nodes over 17 parents", || hints(&inputs, &fake));
    assert_capped_by_budget(a.spent, a.capped, "assets");
    let first = a.hints.module_assets.values().next().expect("a module with assets");
    assert!(first.len() >= 100_000, "{}", first.len());
}

#[test]
fn a_broken_clip_in_a_folder_of_a_thousand_costs_only_itself() {
    // hostile table (hints_of): a broken clip among 1,000; the other 999 are listed (625).
    let names: Vec<String> = (0..1_000).map(|i| format!("clips/c{i}.k7x")).collect();
    let mut files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "source(\"a.wav\")\n")).collect();
    files[500] = (names[500].as_str(), "source(\n");
    let src = views_world::world(&files);
    let roots = views_world::roots();
    let mut cache = Cache::new();
    let inputs =
        song::lanes::inputs::load_inputs(&src, &mut cache, &roots, song::lanes::rig::Rig::default()).expect("inputs");
    let a = hints_of(&src, &mut cache, &roots, &inputs).expect("hints");
    assert_eq!(a.hints.clips.len(), 999);
    assert!(!a.hints.clips.iter().any(|c| c == "c500"));
}

#[cfg(unix)]
mod links {
    use super::*;
    use support::Tmp;

    fn disk_hints(t: &Tmp) -> HintsAnswer {
        let root = support::spell(&t.0);
        let src = song::DiskSource::new(std::slice::from_ref(&root)).expect("source");
        let mut inputs = bare(IndexMap::new());
        inputs.project = t.at("ws/proj");
        hints(&inputs, &src)
    }

    fn folders(a: &HintsAnswer) -> Vec<&str> {
        a.hints.media.keys().map(String::as_str).collect()
    }

    #[test]
    fn a_link_to_its_own_folder_is_listed_once() {
        // decision 630: a folder whose stamp matches an ancestor on the path is not entered.
        let t = Tmp::new("hints-loop");
        t.write("ws/proj/a/deep.wav", "x");
        t.write("ws/proj/a.wav", "x");
        // Aged before the links exist: the aging walk follows links, and these have no end.
        t.age();
        std::os::unix::fs::symlink(".", t.0.join("ws/proj/a/loop")).expect("a link");
        std::os::unix::fs::symlink(".", t.0.join("ws/proj/current")).expect("a link");
        let a = fast("a link loop", || disk_hints(&t));
        assert_eq!(folders(&a), ["", "a"]);
        assert!(!a.capped);
    }

    #[test]
    fn links_to_folders_above_and_to_a_sibling_end_without_the_budget() {
        // decision 630: `up` is the workspace (a mount), `same` the folder itself, `b` a second way into `a`.
        let t = Tmp::new("hints-loops");
        t.write("ws/proj/a/deep.wav", "x");
        t.write("ws/other/elsewhere.wav", "x");
        t.age();
        std::os::unix::fs::symlink("..", t.0.join("ws/proj/a/up")).expect("a link");
        std::os::unix::fs::symlink(".", t.0.join("ws/proj/a/same")).expect("a link");
        std::os::unix::fs::symlink("a", t.0.join("ws/proj/b")).expect("a link");
        let a = fast("two link loops", || disk_hints(&t));
        assert_eq!(folders(&a), ["a", "b"]);
        assert!(!a.capped, "{}", a.spent);
    }

    #[test]
    fn a_folder_link_out_of_the_mounts_is_walked_past_on_disk_and_in_a_replay() {
        // The bundle holds a refused folder as an empty listing, so the replay answers as the disk did.
        let t = Tmp::new("hints-out");
        let outside = Tmp::new("hints-out-target");
        t.write("ws/proj/audio/a.wav", "x");
        std::os::unix::fs::symlink(&outside.0, t.0.join("ws/proj/samples")).expect("a link");
        let root = support::spell(&t.0.join("ws"));
        let disk = song::DiskSource::new(std::slice::from_ref(&root)).expect("source");
        let rec = song::record::Recorder::new(&disk);
        let mut inputs = bare(IndexMap::new());
        inputs.project = t.at("ws/proj");
        let on_disk = hints(&inputs, &rec);
        assert_eq!(folders(&on_disk), ["audio"]);
        let bundle = rec.into_bundle(std::slice::from_ref(&root));
        let mem = song::MemSource::from_bundle(bundle).expect("a bundle");
        let replayed = hints(&inputs, &mem);
        assert_eq!(on_disk.hints.media, replayed.hints.media);
    }
}

#[cfg(not(unix))]
#[test]
fn links_need_a_platform_that_makes_them_cheaply() {
    // A link needs Developer Mode on Windows: the two loops above are named, never skipped silently.
    support::skip("link loops", "a symbolic link is not made without Developer Mode");
}

#[test]
fn a_name_that_could_climb_out_of_its_folder_has_no_link() {
    // decision 629: a name holding `/` has no link anywhere, and one holding `\` or `:` has none under the Windows rule only.
    for rule in [song::PathRule::Posix, song::PathRule::Windows] {
        let mut fake = Fake::default();
        let mut inputs = bare(IndexMap::new());
        inputs.rule = rule;
        for name in ["a/b", "c\\d", "e:f", "plain"] {
            fake.file(&format!("/p/ws/proj/nodes/{name}.k7x"));
            let mut n = LoadedModule::bare(ModuleKind::Plugin, "", name);
            n.home = Some("nodes".to_owned());
            inputs.nodes.nodes.insert(name.to_owned(), n);
        }
        let a = hints(&inputs, &fake);
        let linked: Vec<&str> = a.hints.paths.keys().map(String::as_str).collect();
        let windows = rule.is_windows();
        assert!(linked.contains(&"module:plain"));
        assert!(!linked.contains(&"module:a/b"));
        assert_eq!(linked.contains(&"module:c\\d"), !windows, "{rule:?}");
        assert_eq!(linked.contains(&"module:e:f"), !windows, "{rule:?}");
    }
}
