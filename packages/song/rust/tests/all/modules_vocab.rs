// modules_vocab.rs — the vocabulary as a whole: the built-ins, cold and console folders, DSP sources, the cap and hostile trees.
// A reader would get wrong that the cap weighs the OUTPUT: modules, folders, findings, engine refs copied and entries merged.

use crate::support;

use song::modules::{ModuleKind, Vocabulary, base_module_refs, build_vocabulary_with, builtin_modules};
use song::{Cache, Library, Roots};
use support::{Tmp, disk, mem, plugin_text, roots};

fn vocab(t: &Tmp) -> Vocabulary {
    let r = roots(t);
    build_vocabulary_with(&disk(&r), &mut Cache::new(), &r, 100_000).expect("a vocabulary")
}

// docModule.test.ts, trackModule.test.ts and notSupported: the built-ins' shapes.
#[test]
fn the_builtins_have_the_shapes_the_graph_reads() {
    let b = builtin_modules();
    let doc = &b["@meta/doc"];
    assert_eq!(doc.kind, Some(ModuleKind::Doc));
    assert_eq!(doc.inputs().len(), 1);
    assert_eq!(doc.inputs()[0].mandatory, Some(false));
    assert!(doc.outputs().is_empty());
    let names = |r: &str| b[r].outputs().iter().map(|p| p.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names("@meta/track"), ["audio", "midi"]);
    assert_eq!(names("@meta/strudel-track"), ["audio", "midi"]);
    assert!(names("@meta/group-track").is_empty());
    assert_eq!(names("@meta/picture-track"), ["video", "audio"]);
    assert_eq!(b["@meta/strudel-track"].generated, Some(true));
    assert_eq!(b["@meta/picture-track"].picture, Some(true));
    assert_eq!(b["@meta/track"].generated, None);
    assert!(b["@meta/track"].inputs().is_empty());
    assert_eq!(b["@meta/picture-track"].file, "picture-track");
    let ns = &b["@meta/not-supported"];
    assert_eq!(
        ns.authored.plugin.as_ref().and_then(|p| p.plugin_type.as_deref()),
        Some("not_supported")
    );
    assert_eq!(ns.outputs().len(), 1);
    // The io roles carry their `io` and no outputs at all, as the TypeScript's catalogue leaves them.
    let io = serde_json::to_value(&b["@meta/CAM"]).expect("json");
    assert_eq!(io["io"], "camera");
    assert!(io.get("outputs").is_none());
    assert_eq!(serde_json::to_value(&b["@meta/CUE"]).expect("json")["io"], "out");
    for r in ["IN", "OUT", "CAM", "CUE", "FILE", "MIDI"] {
        assert!(b.contains_key(&format!("@meta/{r}")), "{r}");
    }
}

// coldModules.test.ts, "the cold/ folder": it loads like any, and is no base module; portsOf: own ports over the type's.
#[test]
fn cold_modules_load_but_are_not_base_modules_and_own_ports_win() {
    let t = Tmp::new("tree-cold");
    let lib = "lib/k7-core/modules";
    t.write(&format!("{lib}/cold/separate/module.k7x"), &plugin_text("Sep"));
    t.write(&format!("{lib}/console/half/module.k7x"), &plugin_text("Half"));
    t.write(
        &format!("{lib}/audio/side/module.k7x"),
        "plugin({ type: \"internal\", name: \"Side\" })\ninputs([{ name: \"sidechain\", type: \"audio\", mandatory: false }])\n",
    );
    t.write(&format!("{lib}/audio/plain/module.k7x"), &plugin_text("Plain"));
    let v = vocab(&t);
    let cold = &v.modules["@k7-core/cold/separate"];
    assert_eq!(
        (cold.kind, cold.inputs().len(), cold.outputs().len()),
        (Some(ModuleKind::Plugin), 1, 1)
    );
    let base = base_module_refs(&v.modules);
    assert!(!base.iter().any(|r| r.contains("/cold/") || r.contains("/console/")));
    assert!(base.contains(&"@k7-core/audio/plain".to_owned()));
    assert!(base.windows(2).all(|w| w[0] <= w[1]), "sorted");
    let side = &v.modules["@k7-core/audio/side"];
    assert_eq!(side.inputs()[0].name, "sidechain");
    assert_eq!(side.outputs()[0].name, "out", "the outputs stay the type folder's");
    // Neither an io module, a node nor a lane is a base module.
    assert!(
        !base
            .iter()
            .any(|r| r.starts_with("@meta/IN") || r.starts_with("@meta/track") || r == "@meta/doc")
    );
}

// A jsdsp or faust module names its DSP source beside its own file.
#[test]
fn a_dsp_module_names_its_source_in_its_folder() {
    let t = Tmp::new("tree-dsp");
    t.write(
        "lib/k7-core/modules/audio/js/module.k7x",
        "plugin({ type: \"jsdsp\", name: \"J\" })\n",
    );
    t.write(
        "lib/k7-core/modules/audio/fa/module.k7x",
        "plugin({ type: \"faust\", name: \"F\" })\n",
    );
    let v = vocab(&t);
    assert!(
        v.modules["@k7-core/audio/js"]
            .jsdsp
            .as_deref()
            .is_some_and(|p| p.ends_with("audio/js/jsdsp.ts"))
    );
    assert!(
        v.modules["@k7-core/audio/fa"]
            .faust
            .as_deref()
            .is_some_and(|p| p.ends_with("audio/fa/dsp.dsp"))
    );
}

// Decision 39: the vocabulary's cap counts the OUTPUT: past it the walk stops with one warning naming the root.
#[test]
fn a_tree_past_the_cap_stops_with_one_warning() {
    let t = Tmp::new("tree-cap");
    for i in 0..30 {
        t.write(
            &format!("lib/k7-core/modules/audio/m{i:02}/module.k7x"),
            &plugin_text("M"),
        );
    }
    let r = roots(&t);
    let source = disk(&r);
    let mut cache = Cache::new();
    let n = build_vocabulary_with(&source, &mut cache, &r, 100_000)
        .expect("a vocabulary")
        .modules
        .len();
    let all = build_vocabulary_with(&source, &mut cache, &r, n - 30 + 10).expect("a vocabulary");
    assert!(all.modules.contains_key("@k7-core/audio/m00"));
    let caps: Vec<_> = all
        .findings
        .iter()
        .filter(|f| f.message.contains("more than"))
        .collect();
    assert_eq!(caps.len(), 1, "{:?}", all.findings);
    assert_eq!(caps[0].file, "@k7-core");
    assert!(!all.modules.contains_key("@k7-core/audio/m29"));
}

// A module path past 260 characters reads (Windows' limit is the host's to lift, and a long name is no refusal here).
#[test]
fn a_long_module_path_reads() {
    let t = Tmp::new("tree-long");
    let rel = format!("lib/k7-core/modules/audio/{}", "n".repeat(120));
    t.write(&format!("{rel}/module.k7x"), &plugin_text("Long"));
    let v = vocab(&t);
    assert!(v.modules.contains_key(&format!("@k7-core/audio/{}", "n".repeat(120))));
}

fn mem_roots() -> Roots {
    Roots {
        project: "/p/proj".into(),
        libraries: vec![Library {
            name: "k7-core".into(),
            root: "/p/lib".into(),
            editable: false,
        }],
        ..Roots::default()
    }
}

fn mem_lib(files: &[(String, String)]) -> song::MemSource {
    let prefixed: Vec<(String, &str)> = files
        .iter()
        .map(|(p, t)| (format!("lib/modules/{p}"), t.as_str()))
        .collect();
    mem(&prefixed.iter().map(|(p, t)| (p.as_str(), *t)).collect::<Vec<_>>())
}

/// How many modules the built-ins and the catalogue make with a library that holds nothing.
fn baseline() -> usize {
    let r = mem_roots();
    build_vocabulary_with(&mem_lib(&[]), &mut Cache::new(), &r, 1_000_000)
        .expect("a vocabulary")
        .modules
        .len()
}

// Decision 39: one module with 50,000 engines and 50,000 node files is weighed by what it produces, engine refs copied included.
#[test]
fn a_module_with_fifty_thousand_engines_and_nodes_is_capped() {
    let mut files = vec![("audio/big/module.k7x".to_owned(), plugin_text("Big"))];
    for i in 0..50_000 {
        files.push((format!("audio/big/engines/e{i}/module.k7x"), plugin_text("E")));
        files.push((format!("audio/big/n{i}.k7x"), "content(\"x\")\n".to_owned()));
    }
    let started = std::time::Instant::now();
    let v = build_vocabulary_with(&mem_lib(&files), &mut Cache::new(), &mem_roots(), 100_000).expect("a vocabulary");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
    let caps = v.findings.iter().filter(|f| f.message.contains("more than")).count();
    assert_eq!(caps, 1, "{} findings", v.findings.len());
    let refs: usize = v
        .modules
        .values()
        .map(|m| m.engine_refs.as_ref().map_or(0, indexmap::IndexMap::len))
        .sum();
    assert!(refs <= 300_000, "{refs} engine refs were copied");
    assert!(v.modules.len() <= 150_000, "{} modules", v.modules.len());
}

// Decision 39: findings weigh one each, so a folder of broken files stops the walk as modules do.
#[test]
fn a_folder_of_broken_files_is_capped() {
    let mut files = vec![("audio/m/module.k7x".to_owned(), plugin_text("M"))];
    for i in 0..20_000 {
        files.push((format!("audio/m/n{i}.k7x"), "nonsense(1)\n".to_owned()));
    }
    let cap = baseline() + 1_000;
    let v = build_vocabulary_with(&mem_lib(&files), &mut Cache::new(), &mem_roots(), cap).expect("a vocabulary");
    assert!(v.findings.len() <= 1_100, "{} findings", v.findings.len());
    assert!(v.findings.iter().any(|f| f.message.contains("more than")));
}

// Decision 39 and decision 24: a folder of links out of every mount is capped like any other output.
#[cfg(unix)]
#[test]
fn a_folder_of_outward_links_is_capped() {
    let t = Tmp::new("vocab-links");
    t.write("lib/k7-core/modules/audio/m/module.k7x", &plugin_text("M"));
    t.write("elsewhere/x.k7x", "content(\"x\")\n");
    let folder = t.0.join("lib/k7-core/modules/audio/m");
    let made = (0..3_000)
        .all(|i| std::os::unix::fs::symlink(t.0.join("elsewhere/x.k7x"), folder.join(format!("n{i}.k7x"))).is_ok());
    if !made {
        support::skip("a_folder_of_outward_links_is_capped", "this file system refuses links");
        return;
    }
    let r = roots(&t);
    let cap = baseline() + 500;
    let v = build_vocabulary_with(&disk(&r), &mut Cache::new(), &r, cap).expect("a vocabulary");
    assert!(v.findings.len() <= 600, "{} findings", v.findings.len());
    assert!(v.findings.iter().any(|f| f.message.contains("more than")));
}
